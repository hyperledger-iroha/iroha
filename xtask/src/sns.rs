use std::{
    collections::HashSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use norito::json::{self, JsonDeserialize, JsonSerialize, Value};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::workspace_root;

const SCORECARD_VERSION: u32 = 1;
const HANDOFF_VERSION: u32 = 1;
const DEFAULT_RENEWAL_TARGET: f64 = 0.55;
const DEFAULT_SUPPORT_TARGET: f64 = 0.95;
const SUPPORT_WARNING_DELTA: f64 = 0.05;
const DEFAULT_DISPUTE_TARGET_HOURS: f64 = 168.0; // 7 days
const DISPUTE_WARNING_MULTIPLIER: f64 = 1.5;
const MAX_BASIS_POINTS: u32 = 10_000;
const DEFAULT_SNS_SUFFIXES: &[&str] = &[".sora", ".nexus", ".dao"];
const MONITOR_DEADLINE_DAYS: u32 = 5;
const REPLACEMENT_DEADLINE_DAYS: u32 = 14;
const ADVISORY_ACK_DAYS: i64 = 2;
const STEWARD_PLAYBOOK_PATH: &str = "docs/source/sns/steward_replacement_playbook.md";
const GOVERNANCE_PLAYBOOK_PATH: &str = "docs/source/sns/governance_playbook.md";

#[derive(Clone)]
pub struct CatalogVerifyOptions {
    pub inputs: Vec<PathBuf>,
    pub allow_missing_checksum: bool,
}

#[derive(Debug, JsonDeserialize, Clone)]
struct SuffixCatalogSnapshot {
    version: u32,
    generated_at: String,
    generated_by: Option<String>,
    suffixes: Vec<SuffixCatalogEntry>,
}

#[derive(Debug, JsonDeserialize, Clone)]
struct SuffixCatalogEntry {
    suffix: String,
    suffix_id: u16,
    status: String,
    steward_account: String,
    fund_splitter_account: String,
    payment_asset_id: String,
    referral_cap_bps: u32,
    min_term_years: u32,
    max_term_years: u32,
    grace_period_days: u32,
    redemption_period_days: u32,
    policy_version: u32,
    reserved_labels: Vec<ReservedLabel>,
    pricing: Vec<PricingTier>,
    fee_split: FeeSplit,
}

#[derive(Debug, JsonDeserialize, Clone)]
struct ReservedLabel {
    label: String,
    assigned_to: Option<String>,
    release_at_ms: Option<u64>,
    note: Option<String>,
}

#[derive(Debug, JsonDeserialize, Clone)]
struct PricingTier {
    tier_id: u32,
    label_regex: String,
    base_price: AssetAmount,
    auction_kind: String,
    dutch_floor: Option<AssetAmount>,
    min_duration_years: u32,
    max_duration_years: u32,
}

#[derive(Debug, JsonDeserialize, Clone)]
struct AssetAmount {
    asset_id: String,
    amount: i64,
}

#[derive(Debug, JsonDeserialize, Clone)]
struct FeeSplit {
    treasury_bps: u32,
    steward_bps: u32,
    referral_max_bps: u32,
    escrow_bps: u32,
}

pub fn verify_catalog(opts: CatalogVerifyOptions) -> Result<(), Box<dyn Error>> {
    let CatalogVerifyOptions {
        inputs,
        allow_missing_checksum,
    } = opts;
    let mut targets = if inputs.is_empty() {
        discover_default_catalogs()?
    } else {
        inputs
    };
    if targets.is_empty() {
        return Err(
            "sns-catalog-verify found no catalog snapshots (pass --input to specify one)".into(),
        );
    }
    targets.sort();
    let require_checksum = !allow_missing_checksum;
    for path in targets {
        verify_single_catalog(&path, require_checksum)?;
    }
    Ok(())
}

fn discover_default_catalogs() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = workspace_root().join("docs/examples/sns");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut matches = Vec::new();
    for entry in
        fs::read_dir(&root).map_err(|err| format!("failed to read {}: {err}", root.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("suffix_catalog_"))
        {
            matches.push(path);
        }
    }
    matches.sort();
    Ok(matches)
}

fn verify_single_catalog(path: &Path, require_checksum: bool) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read catalog {}: {err}", path.display()))?;
    let snapshot: SuffixCatalogSnapshot = json::from_slice(&bytes)
        .map_err(|err| format!("failed to parse catalog {} as JSON: {err}", path.display()))?;
    let relative = relative_catalog_path(path)?;
    let mut errors = validate_catalog_snapshot(&snapshot, &relative);
    let digest = sha256_hex(&bytes);

    if require_checksum {
        let checksum_path = path.with_extension("sha256");
        let expected = read_expected_digest(&checksum_path)?;
        if !expected.eq_ignore_ascii_case(&digest) {
            errors.push(format!(
                "checksum mismatch: expected {expected} but computed {digest}"
            ));
        }
    }

    if errors.is_empty() {
        println!(
            "sns catalog {} OK (entries={}, sha256={})",
            relative,
            snapshot.suffixes.len(),
            digest
        );
        Ok(())
    } else {
        let mut message = format!("sns catalog {} failed validation:\n", relative);
        for error in errors {
            message.push_str("  - ");
            message.push_str(&error);
            message.push('\n');
        }
        Err(message.into())
    }
}

#[cfg(test)]
mod portal_stub_tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn portal_stub_writes_expected_content() {
        let dir = tempdir().expect("tempdir");
        let output = dir.path().join("eu-dsa-2026-10.md");
        write_portal_stub(PortalStubOptions {
            cycle: "2026-10".into(),
            suffixes: vec![".sora".into(), "nexus".into()],
            output: output.clone(),
            overwrite: false,
        })
        .expect("portal stub");
        let body = fs::read_to_string(&output).expect("read stub");
        assert!(body.contains("id: regulatory-eu-dsa-2026-10"));
        assert!(body.contains("sidebar_label: EU DSA (Oct 2026)"));
        assert!(body.contains("docs/source/sns/regulatory/eu-dsa/2026-10.md"));
        assert!(body.contains("sns-annex:sora-2026-10:start"));
        assert!(body.contains("sns-annex:nexus-2026-10:start"));
    }

    #[test]
    fn portal_stub_requires_force_and_uses_default_suffixes() {
        let dir = tempdir().expect("tempdir");
        let output = dir.path().join("eu-dsa-2026-11.md");
        fs::write(&output, "existing").expect("write existing");
        let err = write_portal_stub(PortalStubOptions {
            cycle: "2026-11".into(),
            suffixes: Vec::new(),
            output: output.clone(),
            overwrite: false,
        })
        .expect_err("should require force");
        assert!(err.to_string().contains("already exists"));
        write_portal_stub(PortalStubOptions {
            cycle: "2026-11".into(),
            suffixes: Vec::new(),
            output: output.clone(),
            overwrite: true,
        })
        .expect("overwrite portal stub");
        let body = fs::read_to_string(&output).expect("read overwritten");
        assert!(body.contains("sns-annex:dao-2026-11:start"));
    }

    #[test]
    fn portal_annex_block_prefills_front_matter_metadata() {
        let dir = tempdir().expect("tempdir");
        let annex_dir = dir.path().join("docs/source/sns/reports/.sora");
        fs::create_dir_all(&annex_dir).expect("create annex dir");
        let annex_body = r#"---
generated_at: "2026-12-31T01:02:03Z"
dashboard_export:
  path: "artifacts/custom/sns.json"
  sha256: "facefeed"
---
"#;
        fs::write(annex_dir.join("2026-12.md"), annex_body).expect("write annex");

        let block = render_portal_annex_block_at_root(dir.path(), ".sora", "2026-12");
        assert!(
            block.contains("facefeed"),
            "sha should be inlined from front matter"
        );
        assert!(block.contains("2026-12-31T01:02:03Z"));
        assert!(
            block.contains("artifacts/custom/sns.json"),
            "custom dashboard path should be propagated"
        );
    }

    #[test]
    fn portal_annex_block_hashes_artifact_when_available() {
        let dir = tempdir().expect("tempdir");
        let artifact_dir = dir.path().join("artifacts/sns/regulatory/.sora/2026-11");
        fs::create_dir_all(&artifact_dir).expect("create artifact dir");
        let artifact_path = artifact_dir.join("sns_suffix_analytics.json");
        let payload = br#"{"demo":true}"#;
        fs::write(&artifact_path, payload).expect("write dashboard export");

        let block = render_portal_annex_block_at_root(dir.path(), ".sora", "2026-11");
        let expected_sha = sha256_hex(payload);
        assert!(
            block.contains(&expected_sha),
            "artifact hash should replace the placeholder"
        );
        assert!(
            block.contains("artifacts/sns/regulatory/.sora/2026-11/sns_suffix_analytics.json"),
            "default dashboard path should be used when front matter is missing"
        );
        assert!(
            !block.contains("pending"),
            "metadata should be filled when the artifact exists"
        );
    }

    #[test]
    fn portal_annex_block_handles_missing_artifact_without_placeholder() {
        let dir = tempdir().expect("tempdir");
        let block = render_portal_annex_block_at_root(dir.path(), ".sora", "2026-10");
        assert!(
            !block.contains("pending"),
            "annex block should not emit placeholder text when metadata is missing"
        );
        assert!(
            block.contains("unavailable"),
            "annex block should mark missing generated_at as unavailable"
        );
    }
}

fn relative_catalog_path(path: &Path) -> Result<String, Box<dyn Error>> {
    let root = workspace_root();
    let relative = path.strip_prefix(&root).unwrap_or(path);
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn read_expected_digest(path: &Path) -> Result<String, Box<dyn Error>> {
    let body = fs::read_to_string(path)
        .map_err(|err| format!("failed to read checksum {}: {err}", path.display()))?;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let digest = trimmed
            .split_whitespace()
            .next()
            .ok_or_else(|| format!("{}: invalid checksum format", path.display()))?;
        if digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(format!(
                "{}: checksum `{digest}` must contain 64 hexadecimal characters",
                path.display()
            )
            .into());
        }
        return Ok(digest.to_ascii_lowercase());
    }
    Err(format!("{}: checksum file is empty", path.display()).into())
}

fn validate_catalog_snapshot(snapshot: &SuffixCatalogSnapshot, relative_path: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if snapshot.version == 0 {
        errors.push("catalog version must be >= 1".to_string());
    }
    if snapshot.suffixes.is_empty() {
        errors.push("catalog must include at least one suffix entry".to_string());
    }
    if let Err(err) = OffsetDateTime::parse(&snapshot.generated_at, &Rfc3339) {
        errors.push(format!(
            "generated_at `{}` must be RFC3339: {err}",
            snapshot.generated_at
        ));
    }
    match snapshot.generated_by.as_deref() {
        Some(value) => {
            if value != relative_path {
                errors.push(format!(
                    "generated_by `{value}` must match relative path `{relative_path}`"
                ));
            }
        }
        None => errors.push("generated_by field is required".to_string()),
    }

    let mut seen_suffixes = HashSet::new();
    let mut seen_ids = HashSet::new();
    let mut last_id: Option<u16> = None;
    for entry in &snapshot.suffixes {
        if entry.suffix_id == 0 {
            errors.push(format!(
                "suffix `{}` uses id 0x0000 which is reserved",
                entry.suffix
            ));
        }
        if !seen_suffixes.insert(entry.suffix.clone()) {
            errors.push(format!("suffix `{}` appears multiple times", entry.suffix));
        }
        if !seen_ids.insert(entry.suffix_id) {
            errors.push(format!(
                "suffix id 0x{:04X} appears multiple times",
                entry.suffix_id
            ));
        }
        if let Some(prev) = last_id.filter(|&prev| entry.suffix_id <= prev) {
            errors.push(format!(
                "suffix ids must be strictly increasing (0x{:04X} after 0x{:04X})",
                entry.suffix_id, prev
            ));
        }
        last_id = Some(entry.suffix_id);
        errors.extend(validate_suffix_entry(entry));
    }
    errors
}

fn validate_suffix_entry(entry: &SuffixCatalogEntry) -> Vec<String> {
    let mut errors = Vec::new();
    let prefix = format!(
        "suffix {} (id=0x{:04X}, policy v{})",
        entry.suffix, entry.suffix_id, entry.policy_version
    );
    if !is_valid_suffix_literal(&entry.suffix) {
        errors.push(format!(
            "{prefix}: suffix must start with `.` and use lowercase alphanumeric characters"
        ));
    }
    match entry.status.as_str() {
        "active" | "paused" | "revoked" => {}
        other => errors.push(format!(
            "{prefix}: status `{other}` must be active, paused, or revoked"
        )),
    }
    if entry.min_term_years == 0 {
        errors.push(format!("{prefix}: min_term_years must be >= 1"));
    }
    if entry.min_term_years > entry.max_term_years {
        errors.push(format!(
            "{prefix}: min_term_years ({}) exceeds max_term_years ({})",
            entry.min_term_years, entry.max_term_years
        ));
    }
    if entry.grace_period_days == 0 {
        errors.push(format!("{prefix}: grace_period_days must be > 0"));
    }
    if entry.redemption_period_days == 0 {
        errors.push(format!("{prefix}: redemption_period_days must be > 0"));
    }
    ensure_account_literal(
        &entry.steward_account,
        "steward_account",
        &prefix,
        &mut errors,
    );
    ensure_account_literal(
        &entry.fund_splitter_account,
        "fund_splitter_account",
        &prefix,
        &mut errors,
    );
    ensure_asset_literal(
        &entry.payment_asset_id,
        "payment_asset_id",
        &prefix,
        &mut errors,
    );
    if entry.referral_cap_bps > MAX_BASIS_POINTS {
        errors.push(format!(
            "{prefix}: referral_cap_bps {} exceeds 10_000 bps",
            entry.referral_cap_bps
        ));
    }
    if entry.referral_cap_bps > entry.fee_split.referral_max_bps {
        errors.push(format!(
            "{prefix}: referral_cap_bps {} exceeds referral_max_bps {}",
            entry.referral_cap_bps, entry.fee_split.referral_max_bps
        ));
    }
    for (label, value) in [
        ("treasury_bps", entry.fee_split.treasury_bps),
        ("steward_bps", entry.fee_split.steward_bps),
        ("referral_max_bps", entry.fee_split.referral_max_bps),
        ("escrow_bps", entry.fee_split.escrow_bps),
    ] {
        if value > MAX_BASIS_POINTS {
            errors.push(format!("{prefix}: {label} {} exceeds 10_000 bps", value));
        }
    }

    let mut reserved = HashSet::new();
    for label in &entry.reserved_labels {
        if !reserved.insert(label.label.clone()) {
            errors.push(format!(
                "{prefix}: reserved label `{}` appears multiple times",
                label.label
            ));
        }
        ensure_label_literal(&label.label, &prefix, &mut errors);
        if let Some(account) = &label.assigned_to {
            ensure_account_literal(account, "reserved_labels.assigned_to", &prefix, &mut errors);
        }
        if label.release_at_ms.is_some_and(|ts| ts == 0) {
            errors.push(format!(
                "{prefix}: reserved label `{}` release_at_ms must be > 0 when provided",
                label.label
            ));
        }
        if label
            .note
            .as_deref()
            .is_some_and(|note| note.trim().is_empty())
        {
            errors.push(format!(
                "{prefix}: reserved label `{}` note cannot be empty",
                label.label
            ));
        }
    }

    if entry.pricing.is_empty() {
        errors.push(format!("{prefix}: pricing tiers array cannot be empty"));
    } else {
        let mut tier_ids = HashSet::new();
        for tier in &entry.pricing {
            if !tier_ids.insert(tier.tier_id) {
                errors.push(format!(
                    "{prefix}: tier id {} appears multiple times",
                    tier.tier_id
                ));
            }
            validate_pricing_tier(entry, tier, &prefix, &mut errors);
        }
    }

    errors
}

fn validate_pricing_tier(
    entry: &SuffixCatalogEntry,
    tier: &PricingTier,
    prefix: &str,
    errors: &mut Vec<String>,
) {
    if tier.label_regex.trim().is_empty() {
        errors.push(format!(
            "{prefix}: pricing tier {} must include a non-empty label_regex",
            tier.tier_id
        ));
    }
    if tier.min_duration_years == 0 {
        errors.push(format!(
            "{prefix}: pricing tier {} min_duration_years must be >= 1",
            tier.tier_id
        ));
    }
    if tier.min_duration_years < entry.min_term_years {
        errors.push(format!(
            "{prefix}: pricing tier {} min_duration_years {} is below suffix min_term_years {}",
            tier.tier_id, tier.min_duration_years, entry.min_term_years
        ));
    }
    if tier.max_duration_years > entry.max_term_years {
        errors.push(format!(
            "{prefix}: pricing tier {} max_duration_years {} exceeds suffix max_term_years {}",
            tier.tier_id, tier.max_duration_years, entry.max_term_years
        ));
    }
    if tier.min_duration_years > tier.max_duration_years {
        errors.push(format!(
            "{prefix}: pricing tier {} min_duration_years {} exceeds max_duration_years {}",
            tier.tier_id, tier.min_duration_years, tier.max_duration_years
        ));
    }
    if tier.base_price.amount <= 0 {
        errors.push(format!(
            "{prefix}: pricing tier {} base_price amount must be > 0",
            tier.tier_id
        ));
    }
    ensure_asset_literal(
        &tier.base_price.asset_id,
        "pricing.base_price.asset_id",
        prefix,
        errors,
    );
    if tier.base_price.asset_id != entry.payment_asset_id {
        errors.push(format!(
            "{prefix}: pricing tier {} base_price asset `{}` must match payment_asset_id `{}`",
            tier.tier_id, tier.base_price.asset_id, entry.payment_asset_id
        ));
    }

    match tier.auction_kind.as_str() {
        "vickrey_commit_reveal" => {
            if tier.dutch_floor.is_some() {
                errors.push(format!(
                    "{prefix}: pricing tier {} must not set dutch_floor for vickrey auctions",
                    tier.tier_id
                ));
            }
        }
        "dutch_reopen" => match &tier.dutch_floor {
            Some(floor) => {
                if floor.amount <= 0 {
                    errors.push(format!(
                        "{prefix}: pricing tier {} dutch_floor amount must be > 0",
                        tier.tier_id
                    ));
                }
                ensure_asset_literal(
                    &floor.asset_id,
                    "pricing.dutch_floor.asset_id",
                    prefix,
                    errors,
                );
                if floor.asset_id != tier.base_price.asset_id {
                    errors.push(format!(
                        "{prefix}: pricing tier {} dutch_floor asset `{}` must match base_price",
                        tier.tier_id, floor.asset_id
                    ));
                }
            }
            None => errors.push(format!(
                "{prefix}: pricing tier {} must set dutch_floor for dutch_reopen",
                tier.tier_id
            )),
        },
        other => errors.push(format!(
            "{prefix}: pricing tier {} uses unknown auction_kind `{other}`",
            tier.tier_id
        )),
    }
}

fn ensure_account_literal(literal: &str, field: &str, prefix: &str, errors: &mut Vec<String>) {
    let trimmed = literal.trim();
    if trimmed.is_empty() {
        errors.push(format!(
            "{prefix}: {field} must be a non-empty account identifier"
        ));
        return;
    }
    if trimmed.chars().any(char::is_whitespace) {
        errors.push(format!(
            "{prefix}: {field} `{literal}` must not contain whitespace"
        ));
    }
    if let Some((head, tail)) = trimmed.split_once('@')
        && (head.is_empty() || tail.is_empty() || trimmed.matches('@').count() != 1)
    {
        errors.push(format!(
            "{prefix}: {field} `{literal}` must be IH58/uaid/opaque or `<alias|public_key>@domain`"
        ));
    }
}

fn ensure_asset_literal(literal: &str, field: &str, prefix: &str, errors: &mut Vec<String>) {
    if !literal.contains('#') {
        errors.push(format!(
            "{prefix}: {field} `{literal}` must be definition#domain"
        ));
    }
}

fn ensure_label_literal(label: &str, prefix: &str, errors: &mut Vec<String>) {
    if !is_valid_label_literal(label) {
        errors.push(format!(
            "{prefix}: reserved label `{label}` must use lower-case letters, digits, dots, or hyphens"
        ));
    }
}

fn is_valid_suffix_literal(value: &str) -> bool {
    value.starts_with('.')
        && value.len() > 1
        && value
            .chars()
            .skip(1)
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

fn is_valid_label_literal(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '.')
}

#[derive(Clone)]
pub struct ScorecardOptions {
    pub input: PathBuf,
    pub output_json: PathBuf,
    pub output_markdown: Option<PathBuf>,
    pub output_handoff_json: Option<PathBuf>,
    pub output_handoff_markdown: Option<PathBuf>,
    pub handoff_dir: Option<PathBuf>,
}

pub fn generate_scorecard(opts: ScorecardOptions) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(&opts.input)
        .map_err(|err| format!("failed to read {}: {err}", opts.input.display()))?;
    let source: StewardScorecardSource = json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse scorecard input {}: {err}",
            opts.input.display()
        )
    })?;

    let scorecard = build_scorecard(&source)?;
    write_json(&opts.output_json, &scorecard)?;

    if let Some(markdown_path) = &opts.output_markdown {
        let markdown = render_markdown(&scorecard);
        write_text(markdown_path, &markdown)?;
    }

    let needs_handoff = opts.output_handoff_json.is_some()
        || opts.output_handoff_markdown.is_some()
        || opts.handoff_dir.is_some();
    if needs_handoff {
        let packet = build_handoff_packet(
            &scorecard,
            &opts.output_json,
            opts.output_markdown.as_deref(),
        )?;
        if let Some(handoff_json) = &opts.output_handoff_json {
            write_handoff_json(handoff_json, &packet)?;
        }
        if let Some(markdown_path) = &opts.output_handoff_markdown {
            let markdown = render_handoff_markdown(&packet);
            write_text(markdown_path, &markdown)?;
        }
        if let Some(dir) = &opts.handoff_dir {
            write_handoff_directory(dir, &packet)?;
        }
    }

    Ok(())
}

fn write_json(path: &Path, value: &StewardScorecard) -> Result<(), Box<dyn Error>> {
    write_json_value(path, value, "scorecard")
}

fn write_text(path: &Path, body: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body.as_bytes())?;
    Ok(())
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn write_json_value<T: JsonSerialize>(
    path: &Path,
    value: &T,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_value = json::to_value(value).map_err(|err| format!("encode {label} json: {err}"))?;
    let json_text =
        json::to_string_pretty(&json_value).map_err(|err| format!("encode {label} json: {err}"))?;
    fs::write(path, json_text.as_bytes())?;
    Ok(())
}

fn build_scorecard(source: &StewardScorecardSource) -> Result<StewardScorecard, Box<dyn Error>> {
    let generated_at = if let Some(ts) = &source.generated_at {
        ts.clone()
    } else {
        OffsetDateTime::now_utc().format(&Rfc3339)?
    };

    let mut suffixes = Vec::with_capacity(source.suffixes.len());
    for suffix in &source.suffixes {
        suffixes.push(evaluate_suffix(suffix));
    }

    Ok(StewardScorecard {
        version: SCORECARD_VERSION,
        quarter: source.quarter.clone(),
        generated_at,
        suffixes,
    })
}

fn evaluate_suffix(source: &SuffixMetricSource) -> SuffixScorecard {
    let renewal = build_renewal_metric(&source.renewals);
    let support = build_support_metric(&source.support);
    let dispute = build_dispute_metric(&source.disputes);
    let incidents = source.incidents.clone().unwrap_or_default();

    let mut alerts = Vec::new();
    if renewal.status_value != MetricStatus::Pass {
        alerts.push(format!(
            "Renewal retention {:.1}% vs {:.0}% target",
            renewal.renewal_rate * 100.0,
            renewal.target_rate * 100.0
        ));
    }
    if support.status_value != MetricStatus::Pass {
        alerts.push(format!(
            "Support SLA {:.1}% vs {:.0}% target ({} P1 breaches)",
            support.within_sla_rate * 100.0,
            support.target_rate * 100.0,
            support.p1_breach_count
        ));
    }
    if dispute.status_value != MetricStatus::Pass {
        alerts.push(format!(
            "Dispute turnaround {:.0} h vs {:.0} h budget",
            dispute.median_resolution_hours, dispute.target_hours
        ));
    }
    if incidents.guardian_freezes > 0 {
        alerts.push(format!(
            "{} guardian freeze(s) active in quarter",
            incidents.guardian_freezes
        ));
    }

    let rotation = compute_rotation(&renewal, &support, &dispute, &incidents);
    let overall_status = match rotation.level_value {
        RotationLevel::Replace => "critical",
        RotationLevel::Monitor => "watch",
        RotationLevel::None => "healthy",
    };

    SuffixScorecard {
        suffix_id: source.suffix_id,
        suffix: source.suffix.clone(),
        steward: source.steward.clone(),
        metrics: ScorecardMetrics {
            renewal,
            support,
            dispute,
        },
        revenue: source.revenue.clone(),
        incidents,
        alerts,
        rotation,
        overall_status: overall_status.to_string(),
        notes: source.notes.clone().unwrap_or_default(),
    }
}

fn build_renewal_metric(source: &RenewalWindow) -> RenewalMetric {
    let renewed_total = source.renewed_on_time + source.renewed_late;
    let target_rate = source.target_rate.unwrap_or(DEFAULT_RENEWAL_TARGET);
    let renewal_rate = ratio(renewed_total, source.expiring);
    let on_time_rate = ratio(source.renewed_on_time, source.expiring);
    let status = classify_minimum(renewal_rate, target_rate, 0.05);

    RenewalMetric {
        status_value: status,
        status: status.as_str().to_string(),
        expiring: source.expiring,
        renewed_total,
        renewed_on_time: source.renewed_on_time,
        renewed_late: source.renewed_late,
        renewal_rate,
        on_time_rate,
        target_rate,
    }
}

fn build_support_metric(source: &SupportWindow) -> SupportMetric {
    let target_rate = source.target_rate.unwrap_or(DEFAULT_SUPPORT_TARGET);
    let within_sla_rate = ratio(source.tickets_within_sla, source.tickets_total);
    let status = if source.tickets_total == 0 {
        MetricStatus::Pass
    } else if source.p1_breach_count > source.p1_breach_budget.unwrap_or(2) {
        MetricStatus::Breach
    } else {
        classify_minimum(within_sla_rate, target_rate, SUPPORT_WARNING_DELTA)
    };

    SupportMetric {
        status_value: status,
        status: status.as_str().to_string(),
        tickets_total: source.tickets_total,
        tickets_within_sla: source.tickets_within_sla,
        p1_breach_count: source.p1_breach_count,
        median_resolution_hours: source.median_resolution_hours,
        target_rate,
        within_sla_rate,
    }
}

fn build_dispute_metric(source: &DisputeWindow) -> DisputeMetric {
    let target_hours = source.target_hours.unwrap_or(DEFAULT_DISPUTE_TARGET_HOURS);
    let median = source.median_resolution_hours;
    let status = if median <= target_hours {
        MetricStatus::Pass
    } else if median <= target_hours * DISPUTE_WARNING_MULTIPLIER {
        MetricStatus::Warning
    } else {
        MetricStatus::Breach
    };
    DisputeMetric {
        status_value: status,
        status: status.as_str().to_string(),
        cases_opened: source.cases_opened,
        cases_resolved: source.cases_resolved,
        median_resolution_hours: median,
        target_hours,
        sla_breaches: source.sla_breaches,
        backlog: source
            .backlog
            .unwrap_or_else(|| source.cases_opened.saturating_sub(source.cases_resolved)),
    }
}

fn compute_rotation(
    renewal: &RenewalMetric,
    support: &SupportMetric,
    dispute: &DisputeMetric,
    incidents: &IncidentWindow,
) -> RotationRecommendation {
    let statuses = [
        renewal.status_value,
        support.status_value,
        dispute.status_value,
    ];
    let breaches = statuses
        .iter()
        .filter(|status| **status == MetricStatus::Breach)
        .count();
    let warnings = statuses
        .iter()
        .filter(|status| **status == MetricStatus::Warning)
        .count();

    let mut reasons = Vec::new();
    if breaches >= 2 {
        reasons.push("Multiple KPI breaches in the quarter".to_string());
    } else if breaches == 1 {
        reasons.push("Single KPI breach recorded".to_string());
    }
    if warnings > 0 && breaches == 0 {
        reasons.push("KPI warning threshold tripped".to_string());
    }
    if incidents.guardian_freezes > 0 {
        reasons.push(format!(
            "{} guardian freeze(s) opened",
            incidents.guardian_freezes
        ));
    }

    let level = if incidents.guardian_freezes > 0 || breaches >= 2 {
        RotationLevel::Replace
    } else if breaches == 1 || warnings > 0 {
        RotationLevel::Monitor
    } else {
        RotationLevel::None
    };

    RotationRecommendation {
        level_value: level,
        level: level.as_str().to_string(),
        reasons,
    }
}

fn classify_minimum(value: f64, target: f64, warning_delta: f64) -> MetricStatus {
    if value >= target {
        MetricStatus::Pass
    } else if value >= (target - warning_delta) {
        MetricStatus::Warning
    } else {
        MetricStatus::Breach
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn render_markdown(scorecard: &StewardScorecard) -> String {
    let mut body = String::new();
    body.push_str("---\n");
    body.push_str("title: SNS Steward Scorecard\n");
    body.push_str("summary: Automated KPI report for steward monitoring.\n");
    body.push_str("---\n\n");
    body.push_str(&format!(
        "# SNS Steward Scorecard — {}\n\n",
        scorecard.quarter
    ));
    body.push_str(&format!(
        "_Generated at {} (version {})_\n\n",
        scorecard.generated_at, scorecard.version
    ));

    body.push_str("| Suffix | Steward | Renewal | Support SLA | Dispute | Rotation |\n");
    body.push_str("|--------|---------|---------|-------------|---------|----------|\n");
    for suffix in &scorecard.suffixes {
        body.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            suffix.suffix,
            suffix.steward,
            format_metric(&suffix.metrics.renewal),
            format_metric(&suffix.metrics.support),
            format_metric(&suffix.metrics.dispute),
            suffix.rotation.level.as_str()
        ));
    }
    body.push('\n');

    for suffix in &scorecard.suffixes {
        body.push_str(&format!("## {} ({})\n\n", suffix.suffix, suffix.steward));
        body.push_str(&format!(
            "- Renewal retention: {:.1}% (target {:.0}%) — {}\n",
            suffix.metrics.renewal.renewal_rate * 100.0,
            suffix.metrics.renewal.target_rate * 100.0,
            suffix.metrics.renewal.status_value.as_str()
        ));
        body.push_str(&format!(
            "- Support SLA: {:.1}% ({} tickets, {} P1 breaches) — {}\n",
            suffix.metrics.support.within_sla_rate * 100.0,
            suffix.metrics.support.tickets_total,
            suffix.metrics.support.p1_breach_count,
            suffix.metrics.support.status_value.as_str()
        ));
        body.push_str(&format!(
            "- Dispute turnaround: {:.0} h median ({} resolved) — {}\n",
            suffix.metrics.dispute.median_resolution_hours,
            suffix.metrics.dispute.cases_resolved,
            suffix.metrics.dispute.status_value.as_str()
        ));
        body.push_str(&format!(
            "- Rotation posture: {} ({})\n",
            suffix.rotation.level.as_str(),
            if suffix.rotation.reasons.is_empty() {
                "no outstanding actions".to_string()
            } else {
                suffix.rotation.reasons.join("; ")
            }
        ));
        if !suffix.alerts.is_empty() {
            body.push_str("  - Alerts:\n");
            for alert in &suffix.alerts {
                body.push_str(&format!("    - {alert}\n"));
            }
        }
        if !suffix.notes.is_empty() {
            body.push_str("  - Notes:\n");
            for note in &suffix.notes {
                body.push_str(&format!("    - {note}\n"));
            }
        }
        body.push('\n');
    }

    body
}

fn build_handoff_packet(
    scorecard: &StewardScorecard,
    scorecard_json_path: &Path,
    scorecard_markdown_path: Option<&Path>,
) -> Result<StewardHandoffPacket, Box<dyn Error>> {
    let generated_at = OffsetDateTime::parse(&scorecard.generated_at, &Rfc3339).map_err(|err| {
        format!(
            "failed to parse scorecard generated_at `{}` as RFC3339: {err}",
            scorecard.generated_at
        )
    })?;
    let scorecard_path = relative_to_root(scorecard_json_path);
    let scorecard_markdown_path = scorecard_markdown_path.map(relative_to_root);
    let mut motions = Vec::new();

    for suffix in &scorecard.suffixes {
        if suffix.rotation.level_value == RotationLevel::None {
            continue;
        }
        let plan = build_motion_details(suffix, generated_at)?;
        let mut reasons = suffix.rotation.reasons.clone();
        if reasons.is_empty() {
            reasons.extend(suffix.alerts.clone());
        }
        let attachments =
            default_handoff_attachments(&scorecard_path, scorecard_markdown_path.as_deref());

        motions.push(StewardHandoffMotion {
            suffix_id: suffix.suffix_id,
            suffix: suffix.suffix.clone(),
            steward: suffix.steward.clone(),
            rotation: suffix.rotation.level.clone(),
            reasons,
            council_motion: plan.council_motion,
            dao_motion: plan.dao_motion,
            attachments,
            deadline_days: plan.deadline_days,
            deadline_at: Some(plan.deadline_at),
            council_actions: plan.council_actions,
            dao_actions: plan.dao_actions,
        });
    }

    Ok(StewardHandoffPacket {
        version: HANDOFF_VERSION,
        quarter: scorecard.quarter.clone(),
        generated_at: scorecard.generated_at.clone(),
        scorecard_path,
        scorecard_markdown_path,
        playbook_path: STEWARD_PLAYBOOK_PATH.to_string(),
        governance_playbook_path: GOVERNANCE_PLAYBOOK_PATH.to_string(),
        motions,
    })
}

struct MotionPlan {
    council_motion: MotionChecklist,
    dao_motion: MotionChecklist,
    council_actions: Vec<MotionAction>,
    dao_actions: Vec<MotionAction>,
    deadline_days: u32,
    deadline_at: String,
}

fn build_motion_details(
    suffix: &SuffixScorecard,
    generated_at: OffsetDateTime,
) -> Result<MotionPlan, Box<dyn Error>> {
    match suffix.rotation.level_value {
        RotationLevel::Replace => {
            let deadline_at = due_in_days(
                generated_at,
                REPLACEMENT_DEADLINE_DAYS as i64,
                &suffix.suffix,
            )?;
            let council_motion = MotionChecklist {
                title: format!("Council replacement motion for {}", suffix.suffix),
                summary: format!(
                    "Freeze registrar queue, appoint an interim steward for {}, and attach the KPI evidence bundle.",
                    suffix.steward
                ),
                next_steps: vec![
                    "Guardians freeze registrar queue and export registrar/dispute state.".to_string(),
                    "File interim steward motion in the governance tracker with vote window + quorum.".to_string(),
                    "Publish customer FAQ and transparency log entry once voting starts.".to_string(),
                ],
            };
            let dao_motion = MotionChecklist {
                title: format!("DAO ratification for {}", suffix.suffix),
                summary: "Ratify the steward replacement, archive the vote record, and mirror the evidence in the DAO register.".to_string(),
                next_steps: vec![
                    "Post ratification draft referencing the scorecard hash and council motion id.".to_string(),
                    "Record vote outcome, attach FAQ + freeze log, and file in governance addenda.".to_string(),
                ],
            };
            let council_actions = vec![
                MotionAction {
                    owner: "guardian board".to_string(),
                    description:
                        "Freeze registrar queue and export registrar/dispute/state snapshots."
                            .into(),
                    due: due_in_days(generated_at, ADVISORY_ACK_DAYS, &suffix.suffix)?,
                },
                MotionAction {
                    owner: "council chair".to_string(),
                    description:
                        "File interim steward motion with vote window + quorum requirements.".into(),
                    due: due_in_days(generated_at, 7, &suffix.suffix)?,
                },
                MotionAction {
                    owner: "steward ops".to_string(),
                    description:
                        "Publish customer FAQ + transparency log once rotation vote is opened."
                            .into(),
                    due: due_in_days(generated_at, 7, &suffix.suffix)?,
                },
                MotionAction {
                    owner: "council secretary".to_string(),
                    description:
                        "Record vote outcome and schedule hand-off with replacement steward.".into(),
                    due: due_in_days(
                        generated_at,
                        REPLACEMENT_DEADLINE_DAYS as i64,
                        &suffix.suffix,
                    )?,
                },
            ];
            let dao_actions = vec![
                MotionAction {
                    owner: "dao rapporteur".to_string(),
                    description:
                        "Publish ratification draft referencing scorecard hash + council motion id."
                            .into(),
                    due: due_in_days(generated_at, ADVISORY_ACK_DAYS + 1, &suffix.suffix)?,
                },
                MotionAction {
                    owner: "dao secretary".to_string(),
                    description: "Attach FAQ/freeze log to DAO register and archive vote outcome."
                        .into(),
                    due: due_in_days(
                        generated_at,
                        REPLACEMENT_DEADLINE_DAYS as i64,
                        &suffix.suffix,
                    )?,
                },
            ];
            Ok(MotionPlan {
                council_motion,
                dao_motion,
                council_actions,
                dao_actions,
                deadline_days: REPLACEMENT_DEADLINE_DAYS,
                deadline_at,
            })
        }
        RotationLevel::Monitor => {
            let deadline_at =
                due_in_days(generated_at, MONITOR_DEADLINE_DAYS as i64, &suffix.suffix)?;
            let council_motion = MotionChecklist {
                title: format!("Council performance motion for {}", suffix.suffix),
                summary: format!(
                    "Track KPI warnings for {} ({}) and demand a remediation plan within the SN-9 window.",
                    suffix.suffix, suffix.steward
                ),
                next_steps: vec![
                    "Send advisory notice and capture steward acknowledgement (2 business days).".to_string(),
                    "Assign remediation owner + due date in the governance tracker (5 business days).".to_string(),
                    "Schedule follow-up review and attach updated metrics or freeze status.".to_string(),
                ],
            };
            let dao_motion = MotionChecklist {
                title: format!("DAO briefing for {}", suffix.suffix),
                summary: "Share KPI warnings with DAO observers and stage a ratification draft in case escalation is required.".to_string(),
                next_steps: vec![
                    "Publish scorecard hash and remediation summary to DAO observers.".to_string(),
                    "Prepare a pre-filled ratification draft that references the hand-off bundle.".to_string(),
                ],
            };
            let council_actions = vec![
                MotionAction {
                    owner: "council chair".to_string(),
                    description: "Send advisory notice and capture steward acknowledgement.".into(),
                    due: due_in_days(generated_at, ADVISORY_ACK_DAYS, &suffix.suffix)?,
                },
                MotionAction {
                    owner: "council chair".to_string(),
                    description:
                        "Assign remediation owner + due date in governance tracker (business day five)."
                            .into(),
                    due: due_in_days(
                        generated_at,
                        MONITOR_DEADLINE_DAYS as i64,
                        &suffix.suffix,
                    )?,
                },
                MotionAction {
                    owner: "guardian board".to_string(),
                    description: "Schedule follow-up review / freeze check-in.".into(),
                    due: due_in_days(generated_at, 14, &suffix.suffix)?,
                },
            ];
            let dao_actions = vec![
                MotionAction {
                    owner: "dao rapporteur".to_string(),
                    description: "Publish scorecard hash + remediation summary to DAO observers."
                        .into(),
                    due: due_in_days(generated_at, ADVISORY_ACK_DAYS, &suffix.suffix)?,
                },
                MotionAction {
                    owner: "dao secretary".to_string(),
                    description: "Draft ratification template in case escalation is required."
                        .into(),
                    due: due_in_days(generated_at, MONITOR_DEADLINE_DAYS as i64, &suffix.suffix)?,
                },
            ];
            Ok(MotionPlan {
                council_motion,
                dao_motion,
                council_actions,
                dao_actions,
                deadline_days: MONITOR_DEADLINE_DAYS,
                deadline_at,
            })
        }
        RotationLevel::None => {
            Err("cannot build motion details for healthy rotation posture".into())
        }
    }
}

fn due_in_days(
    generated_at: OffsetDateTime,
    days: i64,
    suffix: &str,
) -> Result<String, Box<dyn Error>> {
    let delta = Duration::days(days);
    let due = generated_at
        .checked_add(delta)
        .ok_or_else(|| format!("failed to compute due date (+{days}d) for {suffix}"))?;
    Ok(due.format(&Rfc3339)?)
}

fn default_handoff_attachments(
    scorecard_path: &str,
    scorecard_markdown_path: Option<&str>,
) -> Vec<String> {
    let mut attachments = vec![
        scorecard_path.to_string(),
        STEWARD_PLAYBOOK_PATH.to_string(),
        GOVERNANCE_PLAYBOOK_PATH.to_string(),
    ];
    if let Some(markdown) = scorecard_markdown_path {
        attachments.push(markdown.to_string());
    }
    attachments.sort();
    attachments.dedup();
    attachments
}

fn write_handoff_json(path: &Path, packet: &StewardHandoffPacket) -> Result<(), Box<dyn Error>> {
    write_json_value(path, packet, "hand-off")
}

fn write_handoff_directory(
    dir: &Path,
    packet: &StewardHandoffPacket,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dir)?;
    if packet.motions.is_empty() {
        return Ok(());
    }

    let mut entries = Vec::new();
    for motion in &packet.motions {
        let base = sanitize_suffix_for_file(&motion.suffix);
        let json_path = dir.join(format!("steward_handoff_{base}.json"));
        let markdown_path = dir.join(format!("steward_handoff_{base}.md"));
        let file = StewardHandoffFile {
            version: packet.version,
            quarter: packet.quarter.clone(),
            generated_at: packet.generated_at.clone(),
            scorecard_path: packet.scorecard_path.clone(),
            scorecard_markdown_path: packet.scorecard_markdown_path.clone(),
            playbook_path: packet.playbook_path.clone(),
            governance_playbook_path: packet.governance_playbook_path.clone(),
            motion: motion.clone(),
        };
        write_json_value(&json_path, &file, "hand-off detail")?;
        let markdown = render_single_handoff(&file);
        write_text(&markdown_path, &markdown)?;
        entries.push(HandoffIndexEntry {
            suffix: motion.suffix.clone(),
            steward: motion.steward.clone(),
            rotation: motion.rotation.clone(),
            deadline_days: motion.deadline_days,
            deadline_at: motion.deadline_at.clone(),
            json: relative_to_root(&json_path),
            markdown: relative_to_root(&markdown_path),
        });
    }

    let index = HandoffIndex {
        version: packet.version,
        generated_at: packet.generated_at.clone(),
        quarter: packet.quarter.clone(),
        entries,
    };
    write_json_value(&dir.join("handoff_index.json"), &index, "hand-off index")?;
    Ok(())
}

fn render_handoff_markdown(packet: &StewardHandoffPacket) -> String {
    let mut body = String::new();
    body.push_str("---\n");
    body.push_str("title: SNS Steward Hand-off Packet\n");
    body.push_str(
        "summary: Motions and hand-offs derived from the steward KPI scorecard (SN-9).\n",
    );
    body.push_str("---\n\n");
    body.push_str(&format!(
        "# SNS Steward Hand-off Packet — {}\n\n",
        packet.quarter
    ));
    body.push_str(&format!(
        "_Generated at {} (version {})_\n\n",
        packet.generated_at, packet.version
    ));
    body.push_str("## Inputs\n\n");
    body.push_str(&format!("- Scorecard JSON: `{}`\n", packet.scorecard_path));
    if let Some(markdown) = &packet.scorecard_markdown_path {
        body.push_str(&format!("- Scorecard summary: `{markdown}`\n"));
    }
    body.push_str(&format!("- Steward playbook: `{}`\n", packet.playbook_path));
    body.push_str(&format!(
        "- Governance playbook: `{}`\n\n",
        packet.governance_playbook_path
    ));

    if packet.motions.is_empty() {
        body.push_str("No rotation or remediation motions are required for this quarter.\n");
        return body;
    }

    body.push_str("| Suffix | Steward | Rotation | Deadline (days) | Due (UTC) |\n");
    body.push_str("|--------|---------|---------|-----------------|-----------|\n");
    for motion in &packet.motions {
        body.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            motion.suffix,
            motion.steward,
            motion.rotation,
            motion.deadline_days,
            motion.deadline_at.as_deref().unwrap_or("n/a")
        ));
    }
    body.push('\n');

    for motion in &packet.motions {
        body.push_str(&format!(
            "## {} ({}) — {}\n\n",
            motion.suffix, motion.steward, motion.rotation
        ));
        if !motion.reasons.is_empty() {
            body.push_str("**Reasons:**\n");
            for reason in &motion.reasons {
                body.push_str(&format!("- {reason}\n"));
            }
            body.push('\n');
        }
        let deadline_at = motion
            .deadline_at
            .as_deref()
            .unwrap_or("deadline unavailable");
        body.push_str(&format!(
            "**Deadline:** {} days ({deadline_at})\n\n",
            motion.deadline_days
        ));
        body.push_str("**Council motion**\n");
        body.push_str(&format!("- {}\n", motion.council_motion.title));
        body.push_str(&format!("  - {}\n", motion.council_motion.summary));
        for step in &motion.council_motion.next_steps {
            body.push_str(&format!("  - {}\n", step));
        }
        if !motion.council_actions.is_empty() {
            body.push_str("  - Action owners:\n");
            for action in &motion.council_actions {
                body.push_str(&format!(
                    "    - [{}] {} (due `{}`)\n",
                    action.owner, action.description, action.due
                ));
            }
        }
        body.push('\n');
        body.push_str("**DAO motion**\n");
        body.push_str(&format!("- {}\n", motion.dao_motion.title));
        body.push_str(&format!("  - {}\n", motion.dao_motion.summary));
        for step in &motion.dao_motion.next_steps {
            body.push_str(&format!("  - {}\n", step));
        }
        if !motion.dao_actions.is_empty() {
            body.push_str("  - Action owners:\n");
            for action in &motion.dao_actions {
                body.push_str(&format!(
                    "    - [{}] {} (due `{}`)\n",
                    action.owner, action.description, action.due
                ));
            }
        }
        if !motion.attachments.is_empty() {
            body.push_str("\n**Attachments:**\n");
            for attachment in &motion.attachments {
                body.push_str(&format!("- `{attachment}`\n"));
            }
        }
        body.push('\n');
    }

    body
}

fn render_single_handoff(file: &StewardHandoffFile) -> String {
    let mut body = String::new();
    body.push_str("---\n");
    body.push_str("title: SNS Steward Hand-off\n");
    body.push_str("summary: DAO/council packet derived from the steward KPI scorecard (SN-9).\n");
    body.push_str("---\n\n");
    body.push_str(&format!(
        "# {} ({}) — {}\n\n",
        file.motion.suffix, file.motion.steward, file.motion.rotation
    ));
    body.push_str(&format!(
        "_Generated at {} (version {})_\n\n",
        file.generated_at, file.version
    ));
    body.push_str("## Inputs\n\n");
    body.push_str(&format!("- Scorecard JSON: `{}`\n", file.scorecard_path));
    if let Some(markdown) = &file.scorecard_markdown_path {
        body.push_str(&format!("- Scorecard summary: `{markdown}`\n"));
    }
    body.push_str(&format!("- Steward playbook: `{}`\n", file.playbook_path));
    body.push_str(&format!(
        "- Governance playbook: `{}`\n\n",
        file.governance_playbook_path
    ));
    body.push_str(&format!(
        "**Deadline:** {} days ({})\n\n",
        file.motion.deadline_days,
        file.motion
            .deadline_at
            .as_deref()
            .unwrap_or("deadline unavailable")
    ));
    if !file.motion.reasons.is_empty() {
        body.push_str("**Reasons:**\n");
        for reason in &file.motion.reasons {
            body.push_str(&format!("- {reason}\n"));
        }
        body.push('\n');
    }
    body.push_str("## Council motion\n");
    body.push_str(&format!("- {}\n", file.motion.council_motion.title));
    body.push_str(&format!("  - {}\n", file.motion.council_motion.summary));
    for step in &file.motion.council_motion.next_steps {
        body.push_str(&format!("  - {}\n", step));
    }
    if !file.motion.council_actions.is_empty() {
        body.push_str("  - Action owners:\n");
        for action in &file.motion.council_actions {
            body.push_str(&format!(
                "    - [{}] {} (due `{}`)\n",
                action.owner, action.description, action.due
            ));
        }
    }
    body.push('\n');
    body.push_str("## DAO motion\n");
    body.push_str(&format!("- {}\n", file.motion.dao_motion.title));
    body.push_str(&format!("  - {}\n", file.motion.dao_motion.summary));
    for step in &file.motion.dao_motion.next_steps {
        body.push_str(&format!("  - {}\n", step));
    }
    if !file.motion.dao_actions.is_empty() {
        body.push_str("  - Action owners:\n");
        for action in &file.motion.dao_actions {
            body.push_str(&format!(
                "    - [{}] {} (due `{}`)\n",
                action.owner, action.description, action.due
            ));
        }
    }
    if !file.motion.attachments.is_empty() {
        body.push_str("\n**Attachments:**\n");
        for attachment in &file.motion.attachments {
            body.push_str(&format!("- `{attachment}`\n"));
        }
    }
    body.push('\n');

    body
}

fn format_metric(metric: &impl MetricDisplay) -> String {
    metric.summary_cell()
}

trait MetricDisplay {
    fn summary_cell(&self) -> String;
}

impl MetricDisplay for RenewalMetric {
    fn summary_cell(&self) -> String {
        format!(
            "{} ({:.0}%)",
            self.status_value.as_str(),
            self.renewal_rate * 100.0
        )
    }
}

impl MetricDisplay for SupportMetric {
    fn summary_cell(&self) -> String {
        format!(
            "{} ({:.0}%)",
            self.status_value.as_str(),
            self.within_sla_rate * 100.0
        )
    }
}

impl MetricDisplay for DisputeMetric {
    fn summary_cell(&self) -> String {
        format!(
            "{} ({:.0}h)",
            self.status_value.as_str(),
            self.median_resolution_hours
        )
    }
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
struct StewardScorecardSource {
    quarter: String,
    generated_at: Option<String>,
    suffixes: Vec<SuffixMetricSource>,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
struct SuffixMetricSource {
    suffix_id: u16,
    suffix: String,
    steward: String,
    renewals: RenewalWindow,
    support: SupportWindow,
    disputes: DisputeWindow,
    revenue: RevenueSummary,
    incidents: Option<IncidentWindow>,
    notes: Option<Vec<String>>,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
struct RenewalWindow {
    expiring: u64,
    renewed_on_time: u64,
    renewed_late: u64,
    target_rate: Option<f64>,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
struct SupportWindow {
    tickets_total: u64,
    tickets_within_sla: u64,
    p1_breach_count: u64,
    median_resolution_hours: f64,
    target_rate: Option<f64>,
    p1_breach_budget: Option<u64>,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
struct DisputeWindow {
    cases_opened: u64,
    cases_resolved: u64,
    median_resolution_hours: f64,
    sla_breaches: u64,
    target_hours: Option<f64>,
    backlog: Option<u64>,
}

#[derive(Debug, JsonDeserialize, JsonSerialize, Clone)]
struct RevenueSummary {
    treasury_xor: u64,
    steward_xor: u64,
}

#[derive(Debug, JsonDeserialize, JsonSerialize, Clone, Default)]
struct IncidentWindow {
    guardian_freezes: u64,
    pager_incidents: u64,
}

#[derive(Debug, JsonSerialize)]
struct StewardScorecard {
    version: u32,
    quarter: String,
    generated_at: String,
    suffixes: Vec<SuffixScorecard>,
}

#[derive(Debug, JsonSerialize)]
struct SuffixScorecard {
    suffix_id: u16,
    suffix: String,
    steward: String,
    metrics: ScorecardMetrics,
    revenue: RevenueSummary,
    incidents: IncidentWindow,
    alerts: Vec<String>,
    rotation: RotationRecommendation,
    overall_status: String,
    notes: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct ScorecardMetrics {
    renewal: RenewalMetric,
    support: SupportMetric,
    dispute: DisputeMetric,
}

#[derive(Debug, JsonSerialize)]
struct RenewalMetric {
    #[norito(skip)]
    status_value: MetricStatus,
    status: String,
    expiring: u64,
    renewed_total: u64,
    renewed_on_time: u64,
    renewed_late: u64,
    renewal_rate: f64,
    on_time_rate: f64,
    target_rate: f64,
}

#[derive(Debug, JsonSerialize)]
struct SupportMetric {
    #[norito(skip)]
    status_value: MetricStatus,
    status: String,
    tickets_total: u64,
    tickets_within_sla: u64,
    p1_breach_count: u64,
    median_resolution_hours: f64,
    target_rate: f64,
    within_sla_rate: f64,
}

#[derive(Debug, JsonSerialize)]
struct DisputeMetric {
    #[norito(skip)]
    status_value: MetricStatus,
    status: String,
    cases_opened: u64,
    cases_resolved: u64,
    median_resolution_hours: f64,
    target_hours: f64,
    sla_breaches: u64,
    backlog: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricStatus {
    Pass,
    Warning,
    Breach,
}

impl MetricStatus {
    fn as_str(&self) -> &'static str {
        match self {
            MetricStatus::Pass => "pass",
            MetricStatus::Warning => "warning",
            MetricStatus::Breach => "breach",
        }
    }
}

#[derive(Debug, JsonSerialize)]
struct RotationRecommendation {
    #[norito(skip)]
    level_value: RotationLevel,
    level: String,
    reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RotationLevel {
    None,
    Monitor,
    Replace,
}

impl RotationLevel {
    fn as_str(&self) -> &'static str {
        match self {
            RotationLevel::None => "none",
            RotationLevel::Monitor => "monitor",
            RotationLevel::Replace => "replace",
        }
    }
}

#[derive(Debug, JsonSerialize)]
struct StewardHandoffPacket {
    version: u32,
    quarter: String,
    generated_at: String,
    scorecard_path: String,
    scorecard_markdown_path: Option<String>,
    playbook_path: String,
    governance_playbook_path: String,
    motions: Vec<StewardHandoffMotion>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, Clone)]
struct StewardHandoffMotion {
    suffix_id: u16,
    suffix: String,
    steward: String,
    rotation: String,
    reasons: Vec<String>,
    council_motion: MotionChecklist,
    dao_motion: MotionChecklist,
    attachments: Vec<String>,
    deadline_days: u32,
    deadline_at: Option<String>,
    council_actions: Vec<MotionAction>,
    dao_actions: Vec<MotionAction>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, Clone)]
struct MotionChecklist {
    title: String,
    summary: String,
    next_steps: Vec<String>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, Clone)]
struct MotionAction {
    owner: String,
    description: String,
    due: String,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct StewardHandoffFile {
    version: u32,
    quarter: String,
    generated_at: String,
    scorecard_path: String,
    scorecard_markdown_path: Option<String>,
    playbook_path: String,
    governance_playbook_path: String,
    motion: StewardHandoffMotion,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct HandoffIndex {
    version: u32,
    generated_at: String,
    quarter: String,
    entries: Vec<HandoffIndexEntry>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct HandoffIndexEntry {
    suffix: String,
    steward: String,
    rotation: String,
    deadline_days: u32,
    deadline_at: Option<String>,
    json: String,
    markdown: String,
}

#[derive(Clone)]
pub struct AnnexOptions {
    pub suffix: String,
    pub cycle: String,
    pub dashboard_path: PathBuf,
    pub output_markdown: PathBuf,
    pub dashboard_label: Option<String>,
    pub dashboard_artifact: Option<PathBuf>,
    pub regulatory_entry: Option<PathBuf>,
    pub portal_entry: Option<PathBuf>,
}

#[derive(Clone)]
pub struct PortalStubOptions {
    pub cycle: String,
    pub suffixes: Vec<String>,
    pub output: PathBuf,
    pub overwrite: bool,
}

pub fn generate_annex(opts: AnnexOptions) -> Result<(), Box<dyn Error>> {
    let AnnexOptions {
        suffix,
        cycle,
        dashboard_path,
        output_markdown,
        dashboard_label,
        dashboard_artifact,
        regulatory_entry,
        portal_entry,
    } = opts;

    let bytes = fs::read(&dashboard_path).map_err(|err| {
        format!(
            "failed to read dashboard export {}: {err}",
            dashboard_path.display()
        )
    })?;
    let metadata = parse_dashboard_metadata(&bytes)?;
    let dashboard_sha256 = sha256_hex(&bytes);
    let generated_at = OffsetDateTime::now_utc().format(&Rfc3339)?;
    if let Some(target) = &dashboard_artifact {
        write_bytes(target, &bytes)?;
    }
    let dashboard_path = if let Some(label) = dashboard_label {
        label
    } else if let Some(target) = &dashboard_artifact {
        relative_to_root(target)
    } else {
        relative_to_root(&dashboard_path)
    };
    let regulatory_hint = default_regulatory_hint(&suffix, &cycle);
    let annex_path = relative_to_root(&output_markdown);

    let document = AnnexDocument {
        suffix,
        cycle,
        generated_at,
        annex_path: annex_path.clone(),
        dashboard_path,
        dashboard_sha256,
        dashboard_title: metadata.title,
        dashboard_uid: metadata.uid,
        refresh: metadata.refresh,
        tags: metadata.tags,
        panel_count: metadata.panel_count,
        time_from: metadata.time_from,
        time_to: metadata.time_to,
        templating: metadata.templating,
        regulatory_hint,
    };

    let markdown = render_annex_document(&document);
    write_text(&output_markdown, &markdown)?;
    if let Some(regulatory_path) = regulatory_entry {
        update_regulatory_entry(&regulatory_path, &document)?;
    }
    if let Some(portal_path) = portal_entry {
        update_portal_entry(&portal_path, &document)?;
    }
    Ok(())
}

pub fn default_portal_stub_path(cycle: &str) -> PathBuf {
    workspace_root()
        .join("docs/portal/docs/sns/regulatory")
        .join(format!("eu-dsa-{cycle}.md"))
}

pub fn write_portal_stub(opts: PortalStubOptions) -> Result<(), Box<dyn Error>> {
    let PortalStubOptions {
        cycle,
        suffixes,
        output,
        overwrite,
    } = opts;
    let normalized_suffixes = normalize_suffixes(suffixes);
    if output.exists() && !overwrite {
        return Err(format!(
            "portal memo `{}` already exists (pass --force to overwrite)",
            output.display()
        )
        .into());
    }
    let sidebar_label = sidebar_label_for_cycle(&cycle)?;
    let suffix_scope = normalized_suffixes
        .iter()
        .map(|suffix| format!("  - {suffix}"))
        .collect::<Vec<_>>()
        .join("\n");
    let annex_blocks = normalized_suffixes
        .iter()
        .map(|suffix| render_portal_annex_block(suffix, &cycle))
        .collect::<Vec<_>>()
        .join("\n\n");
    let body = format!(
        "---\nid: regulatory-eu-dsa-{cycle}\ntitle: EU DSA Hosting & Transparency Guidance – Intake Memo\nsidebar_label: EU DSA ({sidebar_label})\ndescription: Regulatory intake memo stub for the SNS EU DSA KPI annex program ({cycle}).\njurisdiction: EU\nregulation: Digital Services Act (EU) – KPI annex program\nsuffix_scope:\n{suffix_scope}\nowners:\n  guardian: guardian-board\n  rapporteur: gov-council-seat-4\n  steward_ack: sora-foundation-suffix-ops\nstatus: scheduled\ncycle: {cycle}\n---\n\n:::note Canonical Source\nThis page mirrors `docs/source/sns/regulatory/eu-dsa/{cycle}.md` and will be updated once the governance memo is final.\n:::\n\n## 1. Intake Summary (Pending)\n\n- **Bulletin:** Pending governance bulletin for cycle {cycle}.\n- **Key requirements:** reserve annex jobs, capture KPI exports, and update localization stubs before submission.\n- **Submission window:** TBD.\n\n## 2. Checklist\n\n1. Append `{cycle}` to `docs/source/sns/regulatory/annex_jobs.json`.\n2. Run `scripts/add_sns_annex_cycle.py {cycle}` to populate annex/resolver stubs.\n3. Replace this stub when the EU DSA memo is finalised and governance publishes the bulletin.\n\n{annex_blocks}\n",
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, body)?;
    println!(
        "wrote sns portal stub {} (suffixes: {})",
        output.display(),
        normalized_suffixes.join(", ")
    );
    Ok(())
}

fn normalize_suffixes(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    let candidates = if values.is_empty() {
        DEFAULT_SNS_SUFFIXES
            .iter()
            .map(|value| (**value).to_string())
            .collect::<Vec<_>>()
    } else {
        values
    };
    for value in candidates {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = if trimmed.starts_with('.') {
            trimmed.to_string()
        } else {
            format!(".{trimmed}")
        };
        if seen.insert(normalized.clone()) {
            result.push(normalized);
        }
    }
    if result.is_empty() {
        DEFAULT_SNS_SUFFIXES
            .iter()
            .map(|value| (**value).to_string())
            .collect()
    } else {
        result
    }
}

fn sidebar_label_for_cycle(cycle: &str) -> Result<String, Box<dyn Error>> {
    let (year, month) = parse_cycle(cycle)?;
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let label = MONTHS
        .get((month - 1) as usize)
        .ok_or_else(|| format!("month {month} is out of range"))?;
    Ok(format!("{label} {year}"))
}

fn parse_cycle(cycle: &str) -> Result<(u32, u32), Box<dyn Error>> {
    let (year_str, month_str) = cycle
        .split_once('-')
        .ok_or_else(|| format!("cycle `{cycle}` must be formatted as YYYY-MM"))?;
    let year: u32 = year_str
        .parse()
        .map_err(|err| format!("invalid year component `{year_str}`: {err}"))?;
    let month: u32 = month_str
        .parse()
        .map_err(|err| format!("invalid month component `{month_str}`: {err}"))?;
    if !(1..=12).contains(&month) {
        return Err(format!("month `{month}` must be between 01 and 12").into());
    }
    Ok((year, month))
}

fn render_portal_annex_block(suffix: &str, cycle: &str) -> String {
    render_portal_annex_block_at_root(&workspace_root(), suffix, cycle)
}

fn render_portal_annex_block_at_root(root: &Path, suffix: &str, cycle: &str) -> String {
    let marker_id = regulatory_marker_id(suffix, cycle);
    let annex_path = format!("docs/source/sns/reports/{suffix}/{cycle}.md");
    let artifact_path =
        format!("artifacts/sns/regulatory/{suffix}/{cycle}/sns_suffix_analytics.json");
    let metadata = portal_annex_metadata(root, &annex_path, &artifact_path);
    let sha = metadata
        .dashboard_sha256
        .unwrap_or_else(|| "unavailable".to_string());
    let generated_at = metadata
        .generated_at
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "<!-- sns-annex:{marker_id}:start -->\n### KPI Dashboard Annex ({suffix} — {cycle})\n\n- Annex report: `{annex_path}`\n- Dashboard export: `{dashboard_path}`\n- Dashboard SHA-256: `{sha}`\n- Generated: `{generated_at}`\n\n<!-- sns-annex:{marker_id}:end -->",
        dashboard_path = metadata.dashboard_path
    )
}

struct PortalAnnexMetadata {
    dashboard_path: String,
    dashboard_sha256: Option<String>,
    generated_at: Option<String>,
}

struct AnnexFrontMatter {
    dashboard_path: Option<String>,
    dashboard_sha256: Option<String>,
    generated_at: Option<String>,
}

fn portal_annex_metadata(
    root: &Path,
    annex_path: &str,
    default_dashboard_path: &str,
) -> PortalAnnexMetadata {
    let annex_fs_path = root.join(annex_path);
    let mut metadata = PortalAnnexMetadata {
        dashboard_path: default_dashboard_path.to_string(),
        dashboard_sha256: None,
        generated_at: None,
    };

    if let Ok(contents) = fs::read_to_string(&annex_fs_path)
        && let Some(front_matter) = parse_annex_front_matter(&contents)
    {
        if let Some(path) = front_matter.dashboard_path {
            metadata.dashboard_path = path;
        }
        metadata.dashboard_sha256 = front_matter.dashboard_sha256;
        metadata.generated_at = front_matter.generated_at;
    }

    let dashboard_fs_path = root.join(&metadata.dashboard_path);
    if metadata.dashboard_sha256.is_none()
        && let Ok(bytes) = fs::read(&dashboard_fs_path)
    {
        metadata.dashboard_sha256 = Some(sha256_hex(&bytes));
    }

    if metadata.generated_at.is_none() {
        metadata.generated_at = modified_timestamp(&annex_fs_path)
            .or_else(|| modified_timestamp(&dashboard_fs_path))
            .or_else(|| Some("unavailable".to_string()));
    }

    metadata
}

fn modified_timestamp(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let datetime: OffsetDateTime = modified.into();
    datetime.format(&Rfc3339).ok()
}

fn parse_annex_front_matter(body: &str) -> Option<AnnexFrontMatter> {
    let mut lines = body.lines();
    if !matches!(lines.next(), Some(line) if line.trim() == "---") {
        return None;
    }

    let mut result = AnnexFrontMatter {
        dashboard_path: None,
        dashboard_sha256: None,
        generated_at: None,
    };
    let mut in_dashboard_export = false;
    for raw_line in lines {
        let trimmed_end = raw_line.trim_end();
        let line = trimmed_end.trim();
        if line == "---" {
            break;
        }
        let indent = trimmed_end.len() - line.len();
        if indent == 0 {
            in_dashboard_export = line.starts_with("dashboard_export:");
        }
        if let Some(value) = line.strip_prefix("generated_at:") {
            result.generated_at = parse_front_matter_value(value);
            continue;
        }
        if in_dashboard_export {
            if let Some(value) = line.strip_prefix("path:") {
                result.dashboard_path = parse_front_matter_value(value);
            } else if let Some(value) = line.strip_prefix("sha256:") {
                result.dashboard_sha256 = parse_front_matter_value(value);
            }
        }
    }

    Some(result)
}

fn parse_front_matter_value(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }

    let unquoted = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value);

    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted.to_string())
    }
}

struct DashboardMetadata {
    title: Option<String>,
    uid: Option<String>,
    refresh: Option<String>,
    tags: Vec<String>,
    panel_count: usize,
    time_from: Option<String>,
    time_to: Option<String>,
    templating: Vec<TemplateVariable>,
}

struct AnnexDocument {
    suffix: String,
    cycle: String,
    generated_at: String,
    annex_path: String,
    dashboard_path: String,
    dashboard_sha256: String,
    dashboard_title: Option<String>,
    dashboard_uid: Option<String>,
    refresh: Option<String>,
    tags: Vec<String>,
    panel_count: usize,
    time_from: Option<String>,
    time_to: Option<String>,
    templating: Vec<TemplateVariable>,
    regulatory_hint: String,
}

#[derive(Clone)]
struct TemplateVariable {
    name: String,
    label: Option<String>,
    selection: Option<String>,
}

fn parse_dashboard_metadata(bytes: &[u8]) -> Result<DashboardMetadata, Box<dyn Error>> {
    let value: Value = json::from_slice(bytes)
        .map_err(|err| format!("failed to parse dashboard export JSON: {err}"))?;

    let title = value
        .get("title")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let uid = value
        .get("uid")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let refresh = value
        .get("refresh")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let panel_count = value
        .get("panels")
        .and_then(Value::as_array)
        .map(|arr| arr.len())
        .unwrap_or(0);
    let tags = value
        .get("tags")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let time_from = value
        .get("time")
        .and_then(|time| time.get("from"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let time_to = value
        .get("time")
        .and_then(|time| time.get("to"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let templating = value
        .get("templating")
        .and_then(|templating| templating.get("list"))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(parse_template_variable).collect())
        .unwrap_or_default();

    Ok(DashboardMetadata {
        title,
        uid,
        refresh,
        tags,
        panel_count,
        time_from,
        time_to,
        templating,
    })
}

fn parse_template_variable(value: &Value) -> Option<TemplateVariable> {
    let name = value.get("name").and_then(Value::as_str)?.to_string();
    let label = value
        .get("label")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let selection = value.get("current").and_then(|current| {
        current
            .get("text")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| display_value(current.get("value")))
    });

    Some(TemplateVariable {
        name,
        label,
        selection,
    })
}

fn render_annex_document(doc: &AnnexDocument) -> String {
    let mut body = String::new();
    body.push_str("---\n");
    body.push_str(&format!("title: SNS KPI Annex ({})\n", doc.suffix));
    body.push_str(&format!(
        "summary: Dashboard evidence bundle for {} cycle {}.\n",
        doc.suffix, doc.cycle
    ));
    body.push_str(&format!("suffix: \"{}\"\n", doc.suffix));
    body.push_str(&format!("cycle: \"{}\"\n", doc.cycle));
    body.push_str(&format!("generated_at: \"{}\"\n", doc.generated_at));
    if let Some(uid) = &doc.dashboard_uid {
        body.push_str(&format!("dashboard_uid: \"{}\"\n", uid));
    }
    if let Some(title) = &doc.dashboard_title {
        body.push_str(&format!("dashboard_title: \"{}\"\n", title));
    }
    if let Some(refresh) = &doc.refresh {
        body.push_str(&format!("dashboard_refresh: \"{}\"\n", refresh));
    }
    body.push_str("dashboard_export:\n");
    body.push_str(&format!("  path: \"{}\"\n", doc.dashboard_path));
    body.push_str(&format!("  sha256: \"{}\"\n", doc.dashboard_sha256));
    body.push_str(&format!(
        "regulatory_annex_target: \"{}\"\n",
        doc.regulatory_hint
    ));
    body.push_str("---\n\n");

    body.push_str(&format!("# {} KPI Annex — {}\n\n", doc.suffix, doc.cycle));
    body.push_str(&format!(
        "_Generated at {} using `{}` (SHA-256 `{}`)._\n\n",
        doc.generated_at, doc.dashboard_path, doc.dashboard_sha256
    ));

    body.push_str("## 1. Dashboard Snapshot\n\n");
    if let Some(title) = &doc.dashboard_title {
        body.push_str(&format!("- Title: `{}`\n", title));
    }
    if let Some(uid) = &doc.dashboard_uid {
        body.push_str(&format!("- UID: `{}`\n", uid));
    }
    if let Some(refresh) = &doc.refresh {
        body.push_str(&format!("- Refresh interval: `{}`\n", refresh));
    }
    if let (Some(from), Some(to)) = (&doc.time_from, &doc.time_to) {
        body.push_str(&format!("- Time range: `{}` → `{}`\n", from, to));
    }
    body.push_str(&format!("- Panels tracked: {}\n", doc.panel_count));
    if !doc.tags.is_empty() {
        let tags = doc
            .tags
            .iter()
            .map(|tag| format!("`{tag}`"))
            .collect::<Vec<_>>()
            .join(", ");
        body.push_str(&format!("- Tags: {}\n", tags));
    }
    body.push('\n');

    if doc.templating.is_empty() {
        body.push_str("No template variables defined in the export.\n\n");
    } else {
        body.push_str("Template variables:\n\n");
        body.push_str("| Name | Selection | Label |\n");
        body.push_str("|------|-----------|-------|\n");
        for var in &doc.templating {
            let selection = var.selection.as_deref().unwrap_or("(not set)");
            let label = var.label.as_deref().unwrap_or("—");
            body.push_str(&format!(
                "| `{}` | `{}` | `{}` |\n",
                var.name, selection, label
            ));
        }
        body.push('\n');
    }

    body.push_str("## 2. Attachments\n\n");
    body.push_str("| Artifact | Path | SHA-256 |\n");
    body.push_str("|----------|------|---------|\n");
    body.push_str(&format!(
        "| Grafana dashboard export | `{}` | `{}` |\n\n",
        doc.dashboard_path, doc.dashboard_sha256
    ));

    body.push_str("## 3. Review Checklist\n\n");
    body.push_str(&format!(
        "- [ ] Attach the dashboard export to `{}` before filing the regulatory memo.\n",
        doc.regulatory_hint
    ));
    body.push_str(
        "- [ ] Capture PDF/CSV snapshots from Grafana and reference them in the annex entry.\n",
    );
    body.push_str("- [ ] Log this file path in the governance tracker once reviewers sign off.\n");

    body
}

fn update_regulatory_entry(path: &Path, doc: &AnnexDocument) -> Result<(), Box<dyn Error>> {
    let mut memo = fs::read_to_string(path)
        .map_err(|err| format!("failed to read regulatory memo {}: {err}", path.display()))?;
    let marker_id = regulatory_marker_id(&doc.suffix, &doc.cycle);
    let start_marker = format!("<!-- sns-annex:{marker_id}:start -->");
    let end_marker = format!("<!-- sns-annex:{marker_id}:end -->");
    let block = render_regulatory_block(doc, &marker_id);
    if let Some(start_idx) = memo.find(&start_marker) {
        let relative_end = memo[start_idx..].find(&end_marker).ok_or_else(|| {
            format!(
                "regulatory memo {} missing end marker {}",
                path.display(),
                end_marker
            )
        })?;
        let end_idx = start_idx + relative_end + end_marker.len();
        memo.replace_range(start_idx..end_idx, &block);
    } else {
        if !memo.ends_with('\n') {
            memo.push('\n');
        }
        memo.push('\n');
        memo.push_str(&block);
    }
    if !memo.ends_with('\n') {
        memo.push('\n');
    }
    write_text(path, &memo)?;
    Ok(())
}

fn update_portal_entry(path: &Path, doc: &AnnexDocument) -> Result<(), Box<dyn Error>> {
    let mut memo = fs::read_to_string(path)
        .map_err(|err| format!("failed to read portal memo {}: {err}", path.display()))?;
    let marker_id = regulatory_marker_id(&doc.suffix, &doc.cycle);
    let start_marker = format!("<!-- sns-annex:{marker_id}:start -->");
    let end_marker = format!("<!-- sns-annex:{marker_id}:end -->");
    let block = render_regulatory_block(doc, &marker_id);
    if let Some(start_idx) = memo.find(&start_marker) {
        let relative_end = memo[start_idx..].find(&end_marker).ok_or_else(|| {
            format!(
                "portal memo {} missing end marker {}",
                path.display(),
                end_marker
            )
        })?;
        let end_idx = start_idx + relative_end + end_marker.len();
        memo.replace_range(start_idx..end_idx, &block);
    } else {
        if !memo.ends_with('\n') {
            memo.push('\n');
        }
        memo.push('\n');
        memo.push_str(&block);
    }
    if !memo.ends_with('\n') {
        memo.push('\n');
    }
    write_text(path, &memo)?;
    Ok(())
}

fn render_regulatory_block(doc: &AnnexDocument, marker_id: &str) -> String {
    format!(
        "<!-- sns-annex:{marker_id}:start -->\n\
### KPI Dashboard Annex ({suffix} — {cycle})\n\
- Annex report: `{annex}`\n\
- Dashboard export: `{dashboard}`\n\
- Dashboard SHA-256: `{sha}`\n\
- Generated: `{generated}`\n\
<!-- sns-annex:{marker_id}:end -->",
        suffix = doc.suffix,
        cycle = doc.cycle,
        annex = doc.annex_path,
        dashboard = doc.dashboard_path,
        sha = doc.dashboard_sha256,
        generated = doc.generated_at
    )
}

fn display_value(value: Option<&Value>) -> Option<String> {
    value.and_then(|v| match v {
        Value::String(text) => Some(text.clone()),
        Value::Number(num) => num
            .as_i64()
            .map(|n| n.to_string())
            .or_else(|| num.as_u64().map(|n| n.to_string()))
            .or_else(|| num.as_f64().map(|n| n.to_string())),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Array(_) | Value::Object(_) => json::to_string(v).ok(),
        Value::Null => None,
    })
}

fn sanitize_suffix_for_file(suffix: &str) -> String {
    let cleaned = suffix
        .trim_start_matches('.')
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let normalized = cleaned.trim_matches('-').to_string();
    if normalized.is_empty() {
        "global".to_string()
    } else {
        normalized
    }
}

fn relative_to_root(path: &Path) -> String {
    let root = workspace_root();
    path.strip_prefix(&root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn default_regulatory_hint(suffix: &str, cycle: &str) -> String {
    let sanitized = suffix.trim_start_matches('.');
    if sanitized.is_empty() {
        format!("docs/source/sns/regulatory/{cycle}.md")
    } else {
        format!("docs/source/sns/regulatory/{sanitized}/{cycle}.md")
    }
}

fn regulatory_marker_id(suffix: &str, cycle: &str) -> String {
    let trimmed = suffix.trim();
    let cleaned = trimmed
        .trim_start_matches('.')
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let normalized = cleaned.trim_matches('-').to_string();
    if normalized.is_empty() {
        format!("global-{cycle}")
    } else {
        format!("{normalized}-{cycle}")
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod catalog_verify_tests {
    use super::*;

    fn sample_pricing() -> PricingTier {
        PricingTier {
            tier_id: 0,
            label_regex: "^[a-z]{3,}$".into(),
            base_price: AssetAmount {
                asset_id: "xor#sora".into(),
                amount: 120,
            },
            auction_kind: "vickrey_commit_reveal".into(),
            dutch_floor: None,
            min_duration_years: 1,
            max_duration_years: 5,
        }
    }

    fn sample_suffix_entry() -> SuffixCatalogEntry {
        SuffixCatalogEntry {
            suffix: ".sora".into(),
            suffix_id: 1,
            status: "active".into(),
            steward_account: "steward@sns".into(),
            fund_splitter_account: "steward@sns".into(),
            payment_asset_id: "xor#sora".into(),
            referral_cap_bps: 500,
            min_term_years: 1,
            max_term_years: 5,
            grace_period_days: 30,
            redemption_period_days: 60,
            policy_version: 1,
            reserved_labels: vec![ReservedLabel {
                label: "treasury".into(),
                assigned_to: Some("treasury@sns".into()),
                release_at_ms: None,
                note: None,
            }],
            pricing: vec![sample_pricing()],
            fee_split: FeeSplit {
                treasury_bps: 7000,
                steward_bps: 3000,
                referral_max_bps: 1000,
                escrow_bps: 0,
            },
        }
    }

    fn sample_snapshot() -> SuffixCatalogSnapshot {
        SuffixCatalogSnapshot {
            version: 1,
            generated_at: "2026-05-01T00:00:00Z".into(),
            generated_by: Some("docs/examples/sns/sample.json".into()),
            suffixes: vec![sample_suffix_entry()],
        }
    }

    #[test]
    fn snapshot_validation_succeeds() {
        let snapshot = sample_snapshot();
        let errors = validate_catalog_snapshot(&snapshot, "docs/examples/sns/sample.json");
        assert!(
            errors.is_empty(),
            "unexpected catalog validation errors: {errors:?}"
        );
    }

    #[test]
    fn duplicate_tier_ids_are_rejected() {
        let mut snapshot = sample_snapshot();
        let mut entry = snapshot.suffixes.pop().expect("entry");
        entry.pricing.push(sample_pricing());
        snapshot.suffixes.push(entry);
        let errors = validate_catalog_snapshot(&snapshot, "docs/examples/sns/sample.json");
        assert!(
            errors
                .iter()
                .any(|err| err.contains("tier id 0 appears multiple times")),
            "expected duplicate-tier validation error, got {errors:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::{NamedTempFile, tempdir};

    use super::*;

    fn sample_source(
        renewal_rate: f64,
        support_rate: f64,
        dispute_hours: f64,
    ) -> StewardScorecardSource {
        StewardScorecardSource {
            quarter: "2026-Q1".to_string(),
            generated_at: Some("2026-04-05T00:00:00Z".to_string()),
            suffixes: vec![SuffixMetricSource {
                suffix_id: 1,
                suffix: ".sora".to_string(),
                steward: "Test Steward".to_string(),
                renewals: RenewalWindow {
                    expiring: 1000,
                    renewed_on_time: (renewal_rate * 1000.0 * 0.8) as u64,
                    renewed_late: (renewal_rate * 1000.0 * 0.2) as u64,
                    target_rate: None,
                },
                support: SupportWindow {
                    tickets_total: 100,
                    tickets_within_sla: (support_rate * 100.0) as u64,
                    p1_breach_count: 1,
                    median_resolution_hours: 4.0,
                    target_rate: None,
                    p1_breach_budget: None,
                },
                disputes: DisputeWindow {
                    cases_opened: 10,
                    cases_resolved: 9,
                    median_resolution_hours: dispute_hours,
                    sla_breaches: 0,
                    target_hours: None,
                    backlog: None,
                },
                revenue: RevenueSummary {
                    treasury_xor: 10,
                    steward_xor: 5,
                },
                incidents: None,
                notes: None,
            }],
        }
    }

    #[test]
    fn scorecard_tracks_statuses() {
        let source = sample_source(0.60, 0.97, 140.0);
        let scorecard = build_scorecard(&source).expect("scorecard");
        let suffix = &scorecard.suffixes[0];
        assert_eq!(suffix.metrics.renewal.status_value, MetricStatus::Pass);
        assert_eq!(suffix.metrics.support.status_value, MetricStatus::Pass);
        assert_eq!(suffix.metrics.dispute.status_value, MetricStatus::Pass);
        assert_eq!(suffix.rotation.level_value, RotationLevel::None);
    }

    #[test]
    fn scorecard_flags_rotation() {
        let source = sample_source(0.30, 0.60, 300.0);
        let scorecard = build_scorecard(&source).expect("scorecard");
        let suffix = &scorecard.suffixes[0];
        assert_eq!(suffix.metrics.renewal.status_value, MetricStatus::Breach);
        assert_eq!(suffix.metrics.dispute.status_value, MetricStatus::Breach);
        assert_eq!(suffix.rotation.level_value, RotationLevel::Replace);
        assert!(!suffix.rotation.reasons.is_empty());
    }

    #[test]
    fn markdown_includes_suffix_details() {
        let source = sample_source(0.70, 0.92, 100.0);
        let scorecard = build_scorecard(&source).expect("scorecard");
        let markdown = render_markdown(&scorecard);
        assert!(markdown.contains("SNS Steward Scorecard"));
        assert!(markdown.contains(".sora"));
        assert!(markdown.contains("Test Steward"));
    }

    #[test]
    fn handoff_packet_includes_rotation_actions() {
        let mut healthy = sample_source(0.70, 0.97, 96.0);
        let healthy_suffix = healthy.suffixes.remove(0);
        let source = StewardScorecardSource {
            quarter: "2026-Q1".to_string(),
            generated_at: Some("2026-04-05T00:00:00Z".to_string()),
            suffixes: vec![
                healthy_suffix,
                SuffixMetricSource {
                    suffix_id: 2,
                    suffix: ".dao".to_string(),
                    steward: "DAO Launchpad Guild".to_string(),
                    renewals: RenewalWindow {
                        expiring: 1600,
                        renewed_on_time: 700,
                        renewed_late: 80,
                        target_rate: Some(0.55),
                    },
                    support: SupportWindow {
                        tickets_total: 120,
                        tickets_within_sla: 60,
                        p1_breach_count: 4,
                        median_resolution_hours: 20.0,
                        target_rate: Some(0.95),
                        p1_breach_budget: Some(1),
                    },
                    disputes: DisputeWindow {
                        cases_opened: 12,
                        cases_resolved: 10,
                        median_resolution_hours: 240.0,
                        sla_breaches: 3,
                        target_hours: Some(168.0),
                        backlog: None,
                    },
                    revenue: RevenueSummary {
                        treasury_xor: 1200,
                        steward_xor: 500,
                    },
                    incidents: Some(IncidentWindow {
                        guardian_freezes: 1,
                        pager_incidents: 1,
                    }),
                    notes: Some(vec![
                        "Escalated support backlog and open guardian freeze.".into(),
                    ]),
                },
            ],
        };
        let scorecard = build_scorecard(&source).expect("scorecard");
        let packet = build_handoff_packet(
            &scorecard,
            Path::new("docs/examples/sns/steward_scorecard_2026q1.json"),
            Some(Path::new(
                "docs/source/sns/reports/steward_scorecard_2026q1.md",
            )),
        )
        .expect("handoff");
        assert_eq!(packet.motions.len(), 1);
        let motion = &packet.motions[0];
        assert_eq!(motion.suffix, ".dao");
        assert_eq!(motion.rotation, "replace");
        assert_eq!(motion.deadline_days, REPLACEMENT_DEADLINE_DAYS);
        assert!(
            motion.council_motion.title.contains("council replacement")
                || motion.council_motion.title.contains(".dao")
        );
        assert!(
            motion
                .attachments
                .iter()
                .any(|path| path.contains("steward_scorecard_2026q1.json"))
        );
        assert!(
            packet
                .scorecard_markdown_path
                .as_deref()
                .is_some_and(|path| path.ends_with("steward_scorecard_2026q1.md"))
        );
    }

    #[test]
    fn handoff_markdown_lists_motions() {
        let source = sample_source(0.35, 0.70, 200.0);
        let scorecard = build_scorecard(&source).expect("scorecard");
        let packet = build_handoff_packet(
            &scorecard,
            Path::new("docs/examples/sns/steward_scorecard_2026q1.json"),
            None,
        )
        .expect("handoff packet");
        let markdown = render_handoff_markdown(&packet);
        assert!(markdown.contains("SNS Steward Hand-off Packet"));
        assert!(markdown.contains("docs/examples/sns/steward_scorecard_2026q1.json"));
        assert!(markdown.contains("Council replacement motion"));
        assert!(markdown.contains("DAO ratification"));
    }

    #[test]
    fn handoff_directory_writes_per_suffix_packets() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("metrics.json");
        let output_json = dir.path().join("scorecard.json");
        let output_markdown = dir.path().join("scorecard.md");
        let handoff_dir = dir.path().join("handoffs");
        let source = sample_source(0.30, 0.60, 300.0);
        let input_value = json::to_value(&source).expect("encode source");
        let input = json::to_string_pretty(&input_value).expect("encode source");
        fs::write(&input_path, input).expect("write input");
        generate_scorecard(ScorecardOptions {
            input: input_path,
            output_json: output_json.clone(),
            output_markdown: Some(output_markdown),
            output_handoff_json: None,
            output_handoff_markdown: None,
            handoff_dir: Some(handoff_dir.clone()),
        })
        .expect("scorecard generation");
        let index_path = handoff_dir.join("handoff_index.json");
        let index_bytes = fs::read(&index_path).expect("read index");
        let index: HandoffIndex = json::from_slice(&index_bytes).expect("decode index");
        assert_eq!(index.entries.len(), 1);
        let entry = &index.entries[0];
        assert_eq!(entry.suffix, ".sora");
        assert_eq!(entry.rotation, "replace");
        let detail_bytes =
            fs::read(handoff_dir.join("steward_handoff_sora.json")).expect("handoff detail");
        let detail: StewardHandoffFile =
            json::from_slice(&detail_bytes).expect("decode handoff detail");
        assert_eq!(detail.motion.rotation, "replace");
        assert!(!detail.motion.council_actions.is_empty());
        assert!(detail.motion.deadline_at.is_some());
        let markdown =
            fs::read_to_string(handoff_dir.join("steward_handoff_sora.md")).expect("read markdown");
        assert!(markdown.contains("DAO motion"));
    }

    #[test]
    fn parse_dashboard_metadata_handles_basic_fields() {
        let json = r#"{
            "title": "SNS KPIs",
            "uid": "sns-kpis",
            "refresh": "1m",
            "tags": ["sns", "governance"],
            "time": {"from": "now-30d", "to": "now"},
            "panels": [{"id":1}],
            "templating": {
                "list": [
                    {"name": "suffix", "label": "Suffix", "current": {"text": "All"}}
                ]
            }
        }"#;
        let metadata = parse_dashboard_metadata(json.as_bytes()).expect("metadata");
        assert_eq!(metadata.title.as_deref(), Some("SNS KPIs"));
        assert_eq!(metadata.uid.as_deref(), Some("sns-kpis"));
        assert_eq!(metadata.refresh.as_deref(), Some("1m"));
        assert_eq!(metadata.panel_count, 1);
        assert_eq!(metadata.tags, vec!["sns", "governance"]);
        assert_eq!(metadata.templating.len(), 1);
        assert_eq!(metadata.templating[0].selection.as_deref(), Some("All"));
    }

    #[test]
    fn render_annex_document_includes_expected_sections() {
        let doc = AnnexDocument {
            suffix: ".sora".into(),
            cycle: "2026-03".into(),
            generated_at: "2026-03-30T00:00:00Z".into(),
            annex_path: "docs/source/sns/reports/.sora/2026-03.md".into(),
            dashboard_path: "dashboards/grafana/sns_suffix_analytics.json".into(),
            dashboard_sha256: "abc123".into(),
            dashboard_title: Some("SNS KPIs".into()),
            dashboard_uid: Some("sns-kpis".into()),
            refresh: Some("1m".into()),
            tags: vec!["sns".into()],
            panel_count: 1,
            time_from: Some("now-30d".into()),
            time_to: Some("now".into()),
            templating: vec![TemplateVariable {
                name: "suffix".into(),
                label: Some("Suffix".into()),
                selection: Some("All".into()),
            }],
            regulatory_hint: "docs/source/sns/regulatory/sora/2026-03.md".into(),
        };
        let markdown = render_annex_document(&doc);
        assert!(markdown.contains(".sora KPI Annex"));
        assert!(markdown.contains("Grafana dashboard export"));
        assert!(markdown.contains("docs/source/sns/regulatory/sora/2026-03.md"));
    }

    #[test]
    fn generate_annex_copies_dashboard_export_when_artifact_specified() {
        use tempfile::tempdir_in;

        let workspace = workspace_root();
        let temp = tempdir_in(&workspace).expect("tempdir");
        let source = temp.path().join("sns_dashboard.json");
        let dashboard_json = r#"{
            "title": "SNS KPIs",
            "uid": "sns-kpis",
            "refresh": "1m",
            "tags": ["sns", "governance"],
            "time": {"from": "now-30d", "to": "now"},
            "panels": [{"id":1}],
            "templating": {"list": []}
        }"#;
        fs::write(&source, dashboard_json).expect("write source");
        let artifact = temp
            .path()
            .join("artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json");
        let output = temp.path().join("annex.md");

        let options = AnnexOptions {
            suffix: ".sora".into(),
            cycle: "2026-03".into(),
            dashboard_path: source.clone(),
            output_markdown: output.clone(),
            dashboard_label: None,
            dashboard_artifact: Some(artifact.clone()),
            regulatory_entry: None,
            portal_entry: None,
        };

        generate_annex(options).expect("annex generation");

        let artifact_bytes = fs::read(&artifact).expect("artifact copy");
        assert_eq!(artifact_bytes, dashboard_json.as_bytes());
        let markdown = fs::read_to_string(&output).expect("annex markdown");
        let expected_path = relative_to_root(&artifact);
        assert!(
            markdown.contains(&expected_path),
            "annex should reference copied artifact path"
        );
    }

    #[test]
    fn regulatory_entry_appends_and_replaces_block() {
        let temp = NamedTempFile::new().expect("temp memo");
        fs::write(temp.path(), "# Memo Header\n").expect("write memo");

        let mut doc = AnnexDocument {
            suffix: ".sora".into(),
            cycle: "2026-03".into(),
            generated_at: "2026-03-05T00:00:00Z".into(),
            annex_path: "docs/source/sns/reports/.sora/2026-03.md".into(),
            dashboard_path: "dashboards/grafana/sns_suffix_analytics.json".into(),
            dashboard_sha256: "deadbeef".into(),
            dashboard_title: None,
            dashboard_uid: None,
            refresh: None,
            tags: vec![],
            panel_count: 0,
            time_from: None,
            time_to: None,
            templating: vec![],
            regulatory_hint: "docs/source/sns/regulatory/sora/2026-03.md".into(),
        };
        update_regulatory_entry(temp.path(), &doc).expect("append block");
        let appended = fs::read_to_string(temp.path()).expect("read memo");
        assert!(
            appended.contains("KPI Dashboard Annex (.sora — 2026-03)"),
            "appended memo should include annex heading"
        );
        assert!(appended.contains("deadbeef"), "sha should be present");

        doc.dashboard_sha256 = "beadfeed".into();
        doc.generated_at = "2026-03-06T12:34:56Z".into();
        update_regulatory_entry(temp.path(), &doc).expect("replace block");
        let replaced = fs::read_to_string(temp.path()).expect("read memo second time");
        assert_eq!(
            replaced
                .matches("KPI Dashboard Annex (.sora — 2026-03)")
                .count(),
            1,
            "block should be replaced in place"
        );
        assert!(
            !replaced.contains("deadbeef") && replaced.contains("beadfeed"),
            "sha should reflect the updated block"
        );
    }
}
