use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use blake3::hash as blake3_hash;
use eyre::{Context, Result, bail, ensure, eyre};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
use norito::json::{Map, Value};
use sha2::{Digest, Sha256};
use time::{Date, format_description::well_known::Iso8601};

use crate::{JsonTarget, workspace_root};

const KIT_FILES: &[(&str, &str)] = &[
    (
        "01-readme.md",
        include_str!("../templates/soranet_testnet/01-readme.md"),
    ),
    (
        "02-checklist.md",
        include_str!("../templates/soranet_testnet/02-checklist.md"),
    ),
    (
        "03-config-example.toml",
        include_str!("../templates/soranet_testnet/03-config-example.toml"),
    ),
    (
        "04-telemetry.md",
        include_str!("../templates/soranet_testnet/04-telemetry.md"),
    ),
    (
        "05-incident-playbook.md",
        include_str!("../templates/soranet_testnet/05-incident-playbook.md"),
    ),
    (
        "06-verification-report.md",
        include_str!("../templates/soranet_testnet/06-verification-report.md"),
    ),
    (
        "07-metrics-sample.json",
        include_str!("../templates/soranet_testnet/07-metrics-sample.json"),
    ),
];

fn value_object<I, K>(entries: I) -> Value
where
    I: IntoIterator<Item = (K, Value)>,
    K: Into<String>,
{
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.into(), value);
    }
    Value::Object(map)
}

#[derive(Clone)]
struct AttachmentDigest {
    label: String,
    path: PathBuf,
    size: u64,
    sha256: String,
    blake3: String,
}

impl AttachmentDigest {
    fn to_value(&self) -> Value {
        value_object([
            ("label", Value::from(self.label.clone())),
            ("path", Value::from(display_path(&self.path))),
            ("size", Value::from(self.size)),
            ("sha256", Value::from(self.sha256.clone())),
            ("blake3", Value::from(self.blake3.clone())),
        ])
    }

    fn to_signing_attachment(&self) -> SigningAttachment {
        SigningAttachment {
            label: self.label.clone(),
            sha256: self.sha256.clone(),
            blake3: self.blake3.clone(),
            bytes: self.size,
            path: display_path(&self.path),
        }
    }
}

pub fn generate_testnet_kit(output_dir: PathBuf) -> Result<()> {
    fs::create_dir_all(&output_dir).context("create output directory")?;
    for (name, contents) in KIT_FILES {
        let path = output_dir.join(name);
        let mut file = File::create(&path).with_context(|| format!("create {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

pub fn evaluate_testnet_metrics(input_path: PathBuf, target: JsonTarget) -> Result<()> {
    let file = File::open(&input_path)
        .with_context(|| format!("open SNNet-10 metrics snapshot {}", input_path.display()))?;
    let value: Value = norito::json::from_reader(file)
        .with_context(|| format!("decode metrics snapshot {}", input_path.display()))?;
    let metrics = MetricsInput::from_value(&value).context("parse SNNet-10 metrics snapshot")?;
    let report = metrics.evaluate();
    let report_value = report.to_value();
    write_report_output(&report_value, target.clone()).context("write SNNet-10 metrics report")?;
    if report.all_passed() {
        Ok(())
    } else {
        bail!("one or more SNNet-10 success metrics fell below the required thresholds");
    }
}

fn write_report_output(value: &Value, target: JsonTarget) -> Result<()> {
    let mut json_text = norito::json::to_string_pretty(value)?;
    json_text.push('\n');
    match target {
        JsonTarget::Stdout => {
            print!("{json_text}");
        }
        JsonTarget::File(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, json_text)?;
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct VerificationAttachment {
    pub label: String,
    pub path: PathBuf,
}

pub struct VerificationFeedOptions {
    pub promotion: String,
    pub window_start: Date,
    pub window_end: Date,
    pub relays: Vec<String>,
    pub metrics_report: PathBuf,
    pub drill_log: Option<PathBuf>,
    pub stage_report: Option<PathBuf>,
    pub attachments: Vec<VerificationAttachment>,
    pub output: JsonTarget,
}

pub struct DrillBundleOptions {
    pub log_path: PathBuf,
    pub signing_key: PathBuf,
    pub promotion: Option<String>,
    pub window_start: Option<Date>,
    pub window_end: Option<Date>,
    pub attachments: Vec<VerificationAttachment>,
    pub output: JsonTarget,
}

pub fn generate_verification_feed(options: VerificationFeedOptions) -> Result<()> {
    ensure!(
        options.window_end >= options.window_start,
        "window end must be on or after the start date"
    );

    ensure!(
        !options.relays.is_empty(),
        "at least one relay id must be provided"
    );

    let mut relays = options
        .relays
        .into_iter()
        .filter(|relay| !relay.trim().is_empty())
        .collect::<Vec<_>>();
    relays.sort();
    relays.dedup();

    let (metrics_value, metrics_sha256, metrics_blake3) =
        read_json_with_hashes(&options.metrics_report).with_context(|| {
            format!(
                "failed to load metrics report from {}",
                options.metrics_report.display()
            )
        })?;

    let drill_entry = if let Some(path) = &options.drill_log {
        let (value, sha256, blake3) = read_json_with_hashes(path)
            .with_context(|| format!("failed to load drill log from {}", path.display()))?;
        let signed = value
            .get("signing")
            .and_then(|entry| entry.get("signature_hex"))
            .is_some();
        Some(value_object([
            ("path", Value::from(display_path(path))),
            ("sha256", Value::from(sha256)),
            ("blake3", Value::from(blake3)),
            ("signed", Value::from(signed)),
            ("value", value),
        ]))
    } else {
        None
    };

    let stage_report_entry = if let Some(path) = &options.stage_report {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read stage report from {}", path.display()))?;
        let bytes = text.as_bytes();
        let sha256 = sha256_hex(bytes);
        let blake3 = blake3_hex(bytes);
        Some(value_object([
            ("path", Value::from(display_path(path))),
            ("bytes", Value::from(bytes.len() as u64)),
            ("sha256", Value::from(sha256)),
            ("blake3", Value::from(blake3)),
            ("contents", Value::from(text)),
        ]))
    } else {
        None
    };

    let attachment_digests = digest_attachments(options.attachments)?;
    let attachments: Vec<Value> = attachment_digests
        .iter()
        .map(AttachmentDigest::to_value)
        .collect();

    let start_iso = format_date(options.window_start)?;
    let end_iso = format_date(options.window_end)?;

    let generated_unix_ms = unix_ms_now();

    let mut root = Map::new();
    root.insert("promotion".into(), Value::from(options.promotion));
    root.insert(
        "window".into(),
        value_object([
            ("start", Value::from(start_iso)),
            ("end", Value::from(end_iso)),
        ]),
    );
    root.insert("generated_unix_ms".into(), Value::from(generated_unix_ms));
    root.insert(
        "relays".into(),
        Value::Array(relays.into_iter().map(Value::from).collect()),
    );
    root.insert(
        "metrics_report".into(),
        value_object([
            ("path", Value::from(display_path(&options.metrics_report))),
            ("sha256", Value::from(metrics_sha256)),
            ("blake3", Value::from(metrics_blake3)),
            ("value", metrics_value),
        ]),
    );
    if let Some(drill) = drill_entry {
        root.insert("drill_log".into(), drill);
    }
    if let Some(stage) = stage_report_entry {
        root.insert("stage_report".into(), stage);
    }
    root.insert("attachments".into(), Value::Array(attachments));

    let feed = Value::Object(root);
    write_report_output(&feed, options.output).context("failed to write SNNet-10 verification feed")
}

struct SigningAttachment {
    label: String,
    sha256: String,
    blake3: String,
    bytes: u64,
    path: String,
}

impl SigningAttachment {
    fn to_value(&self) -> Value {
        value_object([
            ("label", Value::from(self.label.clone())),
            ("path", Value::from(self.path.clone())),
            ("bytes", Value::from(self.bytes)),
            ("sha256", Value::from(self.sha256.clone())),
            ("blake3", Value::from(self.blake3.clone())),
        ])
    }
}

struct SignatureEnvelope {
    algorithm: String,
    public_key_hex: String,
    signature_hex: String,
}

pub fn generate_drill_bundle(options: DrillBundleOptions) -> Result<()> {
    let window = match (options.window_start, options.window_end) {
        (Some(start), Some(end)) => {
            ensure!(
                end >= start,
                "window end must be on or after the start date"
            );
            Some(value_object([
                ("start", Value::from(format_date(start)?)),
                ("end", Value::from(format_date(end)?)),
            ]))
        }
        (None, None) => None,
        _ => bail!("both --window-start and --window-end must be provided together"),
    };

    let log_bytes = fs::read(&options.log_path)
        .with_context(|| format!("failed to read drill log {}", options.log_path.display()))?;
    let log_value: Value = norito::json::from_slice(&log_bytes).with_context(|| {
        format!(
            "failed to decode drill log JSON from {}",
            options.log_path.display()
        )
    })?;
    let log_sha256 = sha256_hex(&log_bytes);
    let log_blake3 = blake3_hex(&log_bytes);
    let log_size = log_bytes.len() as u64;

    let attachment_digests = digest_attachments(options.attachments)?;
    let signing_payload = build_drill_signing_payload(
        options.promotion.as_deref(),
        window.clone(),
        &log_sha256,
        &log_blake3,
        &attachment_digests,
    );
    let signing_payload_bytes = norito::json::to_vec(&signing_payload)?;
    let signature = sign_payload(&signing_payload_bytes, &options.signing_key)?;

    let attachments_value: Vec<Value> = attachment_digests
        .iter()
        .map(AttachmentDigest::to_value)
        .collect();

    let mut root = Map::new();
    root.insert("version".into(), Value::from(1));
    root.insert("generated_unix_ms".into(), Value::from(unix_ms_now()));
    root.insert(
        "drill_log".into(),
        value_object([
            ("path", Value::from(display_path(&options.log_path))),
            ("bytes", Value::from(log_size)),
            ("sha256", Value::from(log_sha256)),
            ("blake3", Value::from(log_blake3)),
            ("value", log_value),
        ]),
    );
    if let Some(promotion) = options.promotion {
        root.insert("promotion".into(), Value::from(promotion));
    }
    if let Some(window_value) = window {
        root.insert("window".into(), window_value);
    }
    root.insert(
        "signing".into(),
        value_object([
            ("algorithm", Value::from(signature.algorithm)),
            ("public_key_hex", Value::from(signature.public_key_hex)),
            ("signature_hex", Value::from(signature.signature_hex)),
            ("payload", signing_payload),
        ]),
    );
    root.insert("attachments".into(), Value::Array(attachments_value));

    let bundle = Value::Object(root);
    write_report_output(&bundle, options.output).context("write signed SNNet-10 drill bundle")
}

fn build_drill_signing_payload(
    promotion: Option<&str>,
    window: Option<Value>,
    log_sha256: &str,
    log_blake3: &str,
    attachments: &[AttachmentDigest],
) -> Value {
    let mut entries = Vec::new();
    entries.push(("version", Value::from(1)));
    entries.push(("log_sha256", Value::from(log_sha256.to_string())));
    entries.push(("log_blake3", Value::from(log_blake3.to_string())));
    if let Some(promotion) = promotion {
        entries.push(("promotion", Value::from(promotion.to_string())));
    }
    if let Some(window) = window {
        entries.push(("window", window));
    }
    let signing_attachments: Vec<Value> = attachments
        .iter()
        .map(AttachmentDigest::to_signing_attachment)
        .map(|attachment| attachment.to_value())
        .collect();
    entries.push(("attachments", Value::Array(signing_attachments)));
    value_object(entries)
}

fn sign_payload(payload: &[u8], signing_key: &Path) -> Result<SignatureEnvelope> {
    let key_text = fs::read_to_string(signing_key)
        .with_context(|| format!("failed to read signing key {}", signing_key.display()))?;
    let cleaned: String = key_text
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let private_key = PrivateKey::from_hex(Algorithm::Ed25519, &cleaned).map_err(|err| {
        eyre!(
            "failed to parse signing key {} as Ed25519 hex: {err}",
            signing_key.display()
        )
    })?;
    let key_pair: KeyPair = private_key.clone().into();
    let signature = Signature::new(key_pair.private_key(), payload);
    let (algorithm, public_bytes) = key_pair.public_key().to_bytes();
    ensure!(
        algorithm == Algorithm::Ed25519,
        "only Ed25519 signing keys are supported"
    );
    Ok(SignatureEnvelope {
        algorithm: "ed25519".to_string(),
        public_key_hex: hex::encode(public_bytes),
        signature_hex: hex::encode(signature.payload()),
    })
}

fn digest_attachments(attachments: Vec<VerificationAttachment>) -> Result<Vec<AttachmentDigest>> {
    let mut seen_labels = HashSet::new();
    let mut digests = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        if !seen_labels.insert(attachment.label.clone()) {
            bail!(
                "duplicate attachment label `{}` supplied; labels must be unique",
                attachment.label
            );
        }
        let bytes = fs::read(&attachment.path).with_context(|| {
            format!("failed to read attachment `{}`", attachment.path.display())
        })?;
        let sha256 = sha256_hex(&bytes);
        let blake3 = blake3_hex(&bytes);
        let size = bytes.len() as u64;
        digests.push(AttachmentDigest {
            label: attachment.label,
            path: attachment.path,
            size,
            sha256,
            blake3,
        });
    }
    digests.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(digests)
}

fn read_json_with_hashes(path: &Path) -> Result<(Value, String, String)> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = norito::json::from_slice(&bytes)
        .with_context(|| format!("failed to decode JSON from {}", path.display()))?;
    let sha256 = sha256_hex(&bytes);
    let blake3 = blake3_hex(&bytes);
    Ok((value, sha256, blake3))
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn blake3_hex(data: &[u8]) -> String {
    blake3_hash(data).to_hex().to_string()
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn display_path(path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(workspace_root()) {
        relative.display().to_string()
    } else {
        path.display().to_string()
    }
}

fn format_date(date: Date) -> Result<String> {
    date.format(&Iso8601::DATE)
        .map_err(|err| eyre!("failed to format date {date}: {err}"))
}

#[derive(Debug)]
struct MetricsInput {
    circuit_completed: f64,
    circuit_brownout: f64,
    circuit_downgrade: f64,
    fetch_total: f64,
    fetch_brownout: f64,
    gar_expected: BTreeMap<String, f64>,
    gar_observed: BTreeMap<String, f64>,
    pow_attempts: f64,
    pow_successes: f64,
    pow_p95_seconds: f64,
    latency_p95_ms: f64,
}

impl MetricsInput {
    fn from_value(value: &Value) -> Result<Self> {
        let root = expect_object(value, "metrics snapshot")?;
        let window_days = number_field(root, "window_days", "metrics")?;
        ensure!(
            window_days > 0.0,
            "metrics.window_days must be greater than zero"
        );

        let circuit = object_field(root, "circuit_events", "metrics")?;
        let circuit_completed =
            non_negative_number(circuit, "completed", "metrics.circuit_events")?;
        let circuit_brownout = non_negative_number(circuit, "brownout", "metrics.circuit_events")?;
        let circuit_downgrade =
            non_negative_number(circuit, "downgrade", "metrics.circuit_events")?;

        let fetch = object_field(root, "fetch_sessions", "metrics")?;
        let fetch_total = non_negative_number(fetch, "total", "metrics.fetch_sessions")?;
        let fetch_brownout = non_negative_number(fetch, "brownout", "metrics.fetch_sessions")?;

        let gar = object_field(root, "gar_mix", "metrics")?;
        let gar_expected = ratio_map_field(
            gar,
            "expected",
            "metrics.gar_mix",
            /*allow_zero=*/ false,
        )?;
        ensure!(
            !gar_expected.is_empty(),
            "metrics.gar_mix.expected must list at least one category"
        );
        let expected_sum: f64 = gar_expected.values().sum();
        ensure!(
            (expected_sum - 1.0).abs() <= 0.001,
            "metrics.gar_mix.expected ratios must sum to 1.0 (found {expected_sum})"
        );
        let gar_observed = count_map_field(
            gar,
            "observed",
            "metrics.gar_mix",
            /*allow_zero=*/ false,
        )?;

        let pow = object_field(root, "pow", "metrics")?;
        let pow_attempts = non_negative_number(pow, "attempts", "metrics.pow")?;
        let pow_successes = non_negative_number(pow, "successes", "metrics.pow")?;
        ensure!(
            pow_successes <= pow_attempts,
            "metrics.pow.successes cannot exceed metrics.pow.attempts"
        );
        let pow_p95_seconds = number_field(pow, "p95_seconds", "metrics.pow")?;
        ensure!(
            pow_p95_seconds >= 0.0,
            "metrics.pow.p95_seconds must be non-negative"
        );

        let latency = object_field(root, "latency_ms", "metrics")?;
        let latency_p95_ms = number_field(latency, "p95", "metrics.latency_ms")?;
        ensure!(
            latency_p95_ms >= 0.0,
            "metrics.latency_ms.p95 must be non-negative"
        );

        Ok(Self {
            circuit_completed,
            circuit_brownout,
            circuit_downgrade,
            fetch_total,
            fetch_brownout,
            gar_expected,
            gar_observed,
            pow_attempts,
            pow_successes,
            pow_p95_seconds,
            latency_p95_ms,
        })
    }

    fn evaluate(&self) -> MetricsReport {
        let mut checks = Vec::new();

        let total_circuits =
            self.circuit_completed + self.circuit_brownout + self.circuit_downgrade;
        let success_ratio = if total_circuits > 0.0 {
            self.circuit_completed / total_circuits
        } else {
            0.0
        };
        let circuits_pass = total_circuits > 0.0 && success_ratio >= 0.95;
        let circuit_details = value_object([
            ("success_ratio", Value::from(success_ratio)),
            ("threshold", Value::from(0.95)),
            (
                "counts",
                value_object([
                    ("completed", Value::from(self.circuit_completed)),
                    ("brownout", Value::from(self.circuit_brownout)),
                    ("downgrade", Value::from(self.circuit_downgrade)),
                    ("total", Value::from(total_circuits)),
                ]),
            ),
        ]);
        checks.push(MetricCheck::new(
            "circuit_success_ratio",
            circuits_pass,
            circuit_details,
        ));

        let brownout_ratio = if self.fetch_total > 0.0 {
            self.fetch_brownout / self.fetch_total
        } else {
            0.0
        };
        let fetch_pass = self.fetch_total > 0.0 && brownout_ratio <= 0.01;
        let fetch_details = value_object([
            ("brownout_ratio", Value::from(brownout_ratio)),
            ("threshold", Value::from(0.01)),
            (
                "counts",
                value_object([
                    ("brownout", Value::from(self.fetch_brownout)),
                    ("total", Value::from(self.fetch_total)),
                ]),
            ),
        ]);
        checks.push(MetricCheck::new(
            "fetch_brownout_ratio",
            fetch_pass,
            fetch_details,
        ));

        let total_observed: f64 = self.gar_observed.values().sum();
        let mut categories = Vec::new();
        let mut max_deviation = 0.0;
        for (category, expected_ratio) in &self.gar_expected {
            let observed_count = self.gar_observed.get(category).copied().unwrap_or(0.0);
            let observed_ratio = if total_observed > 0.0 {
                observed_count / total_observed
            } else {
                0.0
            };
            let deviation = (observed_ratio - expected_ratio).abs();
            if deviation > max_deviation {
                max_deviation = deviation;
            }
            categories.push(value_object([
                ("category", Value::from(category.clone())),
                ("expected_ratio", Value::from(*expected_ratio)),
                ("observed_ratio", Value::from(observed_ratio)),
                ("observed_count", Value::from(observed_count)),
                ("difference", Value::from(deviation)),
            ]));
        }
        let unexpected: Vec<String> = self
            .gar_observed
            .keys()
            .filter(|key| !self.gar_expected.contains_key(*key))
            .cloned()
            .collect();
        let gar_pass = total_observed > 0.0 && unexpected.is_empty() && max_deviation <= 0.10;
        let gar_details = value_object([
            ("max_deviation", Value::from(max_deviation)),
            ("threshold", Value::from(0.10)),
            ("total_observed", Value::from(total_observed)),
            (
                "unexpected_categories",
                Value::Array(unexpected.into_iter().map(Value::from).collect()),
            ),
            ("categories", Value::Array(categories)),
        ]);
        checks.push(MetricCheck::new("gar_mix_variance", gar_pass, gar_details));

        let pow_success_rate = if self.pow_attempts > 0.0 {
            self.pow_successes / self.pow_attempts
        } else {
            0.0
        };
        let pow_pass =
            self.pow_attempts > 0.0 && pow_success_rate >= 0.99 && self.pow_p95_seconds <= 3.0;
        let pow_details = value_object([
            ("success_rate", Value::from(pow_success_rate)),
            ("threshold", Value::from(0.99)),
            ("attempts", Value::from(self.pow_attempts)),
            ("successes", Value::from(self.pow_successes)),
            ("p95_seconds", Value::from(self.pow_p95_seconds)),
            ("p95_threshold", Value::from(3.0)),
        ]);
        checks.push(MetricCheck::new("pow_success_rate", pow_pass, pow_details));

        let latency_pass = self.latency_p95_ms <= 200.0;
        let latency_details = value_object([
            ("p95_ms", Value::from(self.latency_p95_ms)),
            ("threshold_ms", Value::from(200.0)),
        ]);
        checks.push(MetricCheck::new(
            "latency_p95",
            latency_pass,
            latency_details,
        ));

        MetricsReport { checks }
    }
}

struct MetricsReport {
    checks: Vec<MetricCheck>,
}

impl MetricsReport {
    fn all_passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }

    fn to_value(&self) -> Value {
        let checks: Vec<Value> = self.checks.iter().map(MetricCheck::to_value).collect();
        let status = if self.all_passed() { "pass" } else { "fail" };
        value_object([
            ("status", Value::from(status)),
            ("checks", Value::Array(checks)),
        ])
    }
}

struct MetricCheck {
    name: &'static str,
    passed: bool,
    details: Value,
}

impl MetricCheck {
    fn new(name: &'static str, passed: bool, details: Value) -> Self {
        Self {
            name,
            passed,
            details,
        }
    }

    fn to_value(&self) -> Value {
        value_object([
            ("name", Value::from(self.name)),
            ("passed", Value::from(self.passed)),
            ("details", self.details.clone()),
        ])
    }
}

fn expect_object<'a>(value: &'a Value, context: &str) -> Result<&'a Map> {
    value
        .as_object()
        .ok_or_else(|| eyre!("{context} must be a JSON object"))
}

fn object_field<'a>(map: &'a Map, key: &str, parent: &str) -> Result<&'a Map> {
    let value = map
        .get(key)
        .ok_or_else(|| eyre!("missing {parent}.{key}"))?;
    expect_object(value, &format!("{parent}.{key}"))
}

fn number_field(map: &Map, key: &str, parent: &str) -> Result<f64> {
    let value = map
        .get(key)
        .ok_or_else(|| eyre!("missing {parent}.{key}"))?;
    value
        .as_f64()
        .ok_or_else(|| eyre!("{parent}.{key} must be a number"))
}

fn non_negative_number(map: &Map, key: &str, parent: &str) -> Result<f64> {
    let value = number_field(map, key, parent)?;
    ensure!(value >= 0.0, "{parent}.{key} must be non-negative");
    Ok(value)
}

fn ratio_map_field(
    map: &Map,
    key: &str,
    parent: &str,
    allow_zero: bool,
) -> Result<BTreeMap<String, f64>> {
    let value = map
        .get(key)
        .ok_or_else(|| eyre!("missing {parent}.{key}"))?;
    parse_ratio_map(value, &format!("{parent}.{key}"), allow_zero)
}

fn count_map_field(
    map: &Map,
    key: &str,
    parent: &str,
    allow_zero: bool,
) -> Result<BTreeMap<String, f64>> {
    let value = map
        .get(key)
        .ok_or_else(|| eyre!("missing {parent}.{key}"))?;
    parse_count_map(value, &format!("{parent}.{key}"), allow_zero)
}

fn parse_ratio_map(
    value: &Value,
    context: &str,
    allow_zero: bool,
) -> Result<BTreeMap<String, f64>> {
    let map = expect_object(value, context)?;
    let mut result = BTreeMap::new();
    for (key, entry) in map {
        let ratio = entry
            .as_f64()
            .ok_or_else(|| eyre!("{context}.{key} must be a number"))?;
        if !allow_zero {
            ensure!(ratio >= 0.0, "{context}.{key} must be non-negative");
        }
        result.insert(key.clone(), ratio);
    }
    Ok(result)
}

fn parse_count_map(
    value: &Value,
    context: &str,
    allow_zero: bool,
) -> Result<BTreeMap<String, f64>> {
    let map = expect_object(value, context)?;
    let mut result = BTreeMap::new();
    for (key, entry) in map {
        let count = entry
            .as_f64()
            .ok_or_else(|| eyre!("{context}.{key} must be a number"))?;
        if !allow_zero {
            ensure!(count >= 0.0, "{context}.{key} must be non-negative");
        }
        result.insert(key.clone(), count);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::PublicKey;
    use tempfile::TempDir;
    use time::Month;

    use super::*;
    use crate::JsonTarget;

    fn passing_metrics_value() -> Value {
        value_object([
            ("window_days", Value::from(14.0)),
            (
                "circuit_events",
                value_object([
                    ("completed", Value::from(98_500.0)),
                    ("brownout", Value::from(2_400.0)),
                    ("downgrade", Value::from(600.0)),
                ]),
            ),
            (
                "fetch_sessions",
                value_object([
                    ("total", Value::from(24_000.0)),
                    ("brownout", Value::from(120.0)),
                ]),
            ),
            (
                "gar_mix",
                value_object([
                    (
                        "expected",
                        value_object([
                            ("anon_guard_pq", Value::from(0.55)),
                            ("anon_majority_pq", Value::from(0.30)),
                            ("anon_strict_pq", Value::from(0.15)),
                        ]),
                    ),
                    (
                        "observed",
                        value_object([
                            ("anon_guard_pq", Value::from(13_100.0)),
                            ("anon_majority_pq", Value::from(7_200.0)),
                            ("anon_strict_pq", Value::from(3_600.0)),
                        ]),
                    ),
                ]),
            ),
            (
                "pow",
                value_object([
                    ("attempts", Value::from(15_000.0)),
                    ("successes", Value::from(14_970.0)),
                    ("p95_seconds", Value::from(2.6)),
                ]),
            ),
            ("latency_ms", value_object([("p95", Value::from(182.0))])),
        ])
    }

    #[test]
    fn metrics_report_passes_when_within_thresholds() {
        let metrics = MetricsInput::from_value(&passing_metrics_value()).expect("parse metrics");
        let report = metrics.evaluate();
        assert!(report.all_passed(), "expected report to pass");
        let value = report.to_value();
        assert_eq!(value["status"], Value::from("pass"));
        assert_eq!(value["checks"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn metrics_report_fails_on_latency_regression() {
        let mut failing = passing_metrics_value();
        if let Value::Object(ref mut root) = failing
            && let Some(Value::Object(latency)) = root.get_mut("latency_ms")
        {
            latency.insert(String::from("p95"), Value::from(245.0));
        }
        let metrics = MetricsInput::from_value(&failing).expect("parse metrics");
        let report = metrics.evaluate();
        assert!(!report.all_passed(), "expected latency failure");
        let value = report.to_value();
        assert_eq!(value["status"], Value::from("fail"));
        let latency_entry = value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == Value::from("latency_p95"))
            .expect("latency check present");
        assert_eq!(latency_entry["details"]["p95_ms"], Value::from(245.0));
    }

    #[test]
    fn evaluate_testnet_metrics_writes_report_and_errors_on_failure() {
        let temp = TempDir::new().expect("temp dir");
        let input_path = temp.path().join("metrics.json");
        let failing = value_object([
            ("window_days", Value::from(14.0)),
            (
                "circuit_events",
                value_object([
                    ("completed", Value::from(900.0)),
                    ("brownout", Value::from(150.0)),
                    ("downgrade", Value::from(50.0)),
                ]),
            ),
            (
                "fetch_sessions",
                value_object([
                    ("total", Value::from(1_200.0)),
                    ("brownout", Value::from(40.0)),
                ]),
            ),
            (
                "gar_mix",
                value_object([
                    (
                        "expected",
                        value_object([
                            ("anon_guard_pq", Value::from(0.6)),
                            ("anon_majority_pq", Value::from(0.25)),
                            ("anon_strict_pq", Value::from(0.15)),
                        ]),
                    ),
                    (
                        "observed",
                        value_object([
                            ("anon_guard_pq", Value::from(600.0)),
                            ("anon_majority_pq", Value::from(300.0)),
                            ("anon_strict_pq", Value::from(100.0)),
                        ]),
                    ),
                ]),
            ),
            (
                "pow",
                value_object([
                    ("attempts", Value::from(500.0)),
                    ("successes", Value::from(480.0)),
                    ("p95_seconds", Value::from(3.4)),
                ]),
            ),
            ("latency_ms", value_object([("p95", Value::from(210.0))])),
        ]);
        let mut json_text = norito::json::to_string_pretty(&failing).expect("serialize metrics");
        json_text.push('\n');
        fs::write(&input_path, json_text).expect("write metrics input");
        let report_path = temp.path().join("report.json");
        let result =
            evaluate_testnet_metrics(input_path.clone(), JsonTarget::File(report_path.clone()));
        assert!(result.is_err(), "expected metrics evaluation to fail");
        let report_text = fs::read_to_string(&report_path).expect("read report");
        let report_value: Value = norito::json::from_str(&report_text).expect("parse report json");
        assert_eq!(report_value["status"], Value::from("fail"));
    }

    #[test]
    fn verification_feed_includes_expected_fields() {
        let temp = TempDir::new().expect("temp dir");
        let metrics_path = temp.path().join("metrics.json");
        let metrics_value = passing_metrics_value();
        let mut metrics_text = norito::json::to_string_pretty(&metrics_value).unwrap();
        metrics_text.push('\n');
        fs::write(&metrics_path, metrics_text).unwrap();

        let drill_path = temp.path().join("drills.json");
        fs::write(
            &drill_path,
            r#"[{"timestamp":"2026-11-05T03:17:00Z","region":"NRT","event":"brownout"}]"#,
        )
        .unwrap();

        let stage_report = temp.path().join("stage.md");
        fs::write(&stage_report, "# Report\nAll good.\n").unwrap();

        let attachment_path = temp.path().join("exit_bond.to");
        fs::write(&attachment_path, b"bond-data").unwrap();

        let feed_path = temp.path().join("feed.json");
        let options = VerificationFeedOptions {
            promotion: "T0->T1".into(),
            window_start: Date::from_calendar_date(2026, Month::November, 1).unwrap(),
            window_end: Date::from_calendar_date(2026, Month::November, 14).unwrap(),
            relays: vec!["relay-sjc-01".into(), "relay-nrt-02".into()],
            metrics_report: metrics_path.clone(),
            drill_log: Some(drill_path.clone()),
            stage_report: Some(stage_report.clone()),
            attachments: vec![VerificationAttachment {
                label: "exit-bond".into(),
                path: attachment_path.clone(),
            }],
            output: JsonTarget::File(feed_path.clone()),
        };

        generate_verification_feed(options).expect("generate feed");

        let feed_text = fs::read_to_string(&feed_path).expect("read feed");
        let feed_value: Value = norito::json::from_str(&feed_text).expect("parse feed");
        assert_eq!(feed_value["promotion"], Value::from("T0->T1"));
        assert_eq!(
            feed_value["metrics_report"]["path"],
            Value::from(metrics_path.display().to_string())
        );
        assert!(feed_value["metrics_report"]["blake3"].is_string());
        let relays = feed_value["relays"].as_array().unwrap();
        assert_eq!(relays.len(), 2);
        assert!(relays.contains(&Value::from("relay-sjc-01")));
        let drill = feed_value["drill_log"].as_object().expect("drill log");
        assert!(drill["blake3"].is_string());
        assert_eq!(drill["signed"], Value::from(false));
        let stage = feed_value["stage_report"]
            .as_object()
            .expect("stage report");
        assert!(stage["bytes"].is_number());
        assert!(stage["blake3"].is_string());
        let attachments = feed_value["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0]["label"], Value::from("exit-bond"));
        assert_eq!(
            attachments[0]["path"],
            Value::from(attachment_path.display().to_string())
        );
        assert!(attachments[0]["blake3"].is_string());
    }

    #[test]
    fn drill_bundle_signs_and_feed_marks_signed() {
        let temp = TempDir::new().expect("temp dir");
        let metrics_input = temp.path().join("metrics.json");
        let mut metrics_text =
            norito::json::to_string_pretty(&passing_metrics_value()).expect("serialize metrics");
        metrics_text.push('\n');
        fs::write(&metrics_input, metrics_text).expect("write metrics input");
        let metrics_report = temp.path().join("metrics-report.json");
        evaluate_testnet_metrics(
            metrics_input.clone(),
            JsonTarget::File(metrics_report.clone()),
        )
        .expect("evaluate metrics");

        let drill_log_path = temp.path().join("drills.json");
        fs::write(
            &drill_log_path,
            r#"[{"timestamp":"2026-11-05T03:17:00Z","region":"NRT","event":"brownout","alert_id":"alert://soranet/brownout/1234"}]"#,
        )
        .expect("write drills");
        let guard_path = temp.path().join("guard.log");
        fs::write(&guard_path, "rotation-ok").expect("write guard log");
        let exit_path = temp.path().join("exit-bond.to");
        fs::write(&exit_path, "bond-data").expect("write exit bond");

        let signing_key_path = temp.path().join("signing.key");
        let signing_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let signing_hex = hex::encode(signing_pair.private_key().to_bytes().1);
        fs::write(&signing_key_path, signing_hex).expect("write signing key");

        let window_start = Date::from_calendar_date(2026, Month::November, 1).unwrap();
        let window_end = Date::from_calendar_date(2026, Month::November, 14).unwrap();
        let bundle_path = temp.path().join("drills-signed.json");
        let bundle_options = DrillBundleOptions {
            log_path: drill_log_path.clone(),
            signing_key: signing_key_path.clone(),
            promotion: Some("T0->T1".into()),
            window_start: Some(window_start),
            window_end: Some(window_end),
            attachments: vec![
                VerificationAttachment {
                    label: "guard-rotation".into(),
                    path: guard_path.clone(),
                },
                VerificationAttachment {
                    label: "exit-bond".into(),
                    path: exit_path.clone(),
                },
            ],
            output: JsonTarget::File(bundle_path.clone()),
        };
        generate_drill_bundle(bundle_options).expect("drill bundle");

        let bundle_text = fs::read_to_string(&bundle_path).expect("read bundle");
        let bundle_value: Value = norito::json::from_str(&bundle_text).expect("parse bundle json");
        let payload = bundle_value["signing"]["payload"].clone();
        let payload_bytes = norito::json::to_vec(&payload).expect("payload bytes");
        let signature_hex = bundle_value["signing"]["signature_hex"]
            .as_str()
            .expect("signature hex");
        let public_hex = bundle_value["signing"]["public_key_hex"]
            .as_str()
            .expect("public key hex");
        let signature =
            Signature::from_bytes(&hex::decode(signature_hex).expect("decode signature bytes"));
        let public_key =
            PublicKey::from_hex(Algorithm::Ed25519, public_hex).expect("parse public key");
        signature
            .verify(&public_key, &payload_bytes)
            .expect("signature must verify");

        let stage_report_path = temp.path().join("stage.md");
        fs::write(&stage_report_path, "# Stage report\n").expect("write stage report");
        let feed_path = temp.path().join("feed.json");
        let feed_options = VerificationFeedOptions {
            promotion: "T0->T1".into(),
            window_start,
            window_end,
            relays: vec!["relay-sjc-01".into()],
            metrics_report: metrics_report.clone(),
            drill_log: Some(bundle_path.clone()),
            stage_report: Some(stage_report_path.clone()),
            attachments: vec![VerificationAttachment {
                label: "bundle".into(),
                path: bundle_path.clone(),
            }],
            output: JsonTarget::File(feed_path.clone()),
        };
        generate_verification_feed(feed_options).expect("generate feed");
        let feed_text = fs::read_to_string(feed_path).expect("read feed");
        let feed_value: Value = norito::json::from_str(&feed_text).expect("parse feed");
        assert_eq!(feed_value["drill_log"]["signed"], Value::from(true));
        let bundle_file_sha =
            sha256_hex(&fs::read(&bundle_path).expect("drill bundle file should remain readable"));
        assert_eq!(
            feed_value["drill_log"]["sha256"],
            Value::from(bundle_file_sha)
        );
        assert_eq!(
            feed_value["drill_log"]["value"]["drill_log"]["sha256"],
            bundle_value["drill_log"]["sha256"]
        );
    }
}
