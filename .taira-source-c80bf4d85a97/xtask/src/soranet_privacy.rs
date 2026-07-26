use std::{
    collections::BTreeMap,
    convert::TryFrom,
    error::Error,
    fmt::Write as _,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha_data_model::soranet::privacy_metrics::{
    SoranetPrivacyBucketMetricsV1, SoranetPrivacyEventHandshakeFailureV1,
    SoranetPrivacyEventHandshakeSuccessV1, SoranetPrivacyEventKindV1, SoranetPrivacyEventV1,
    SoranetPrivacyHandshakeFailureV1, SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1,
    SoranetPrivacySuppressionReasonV1,
};
use iroha_telemetry::privacy::{PrivacyBucketConfig, PrivacyConfigError, SoranetSecureAggregator};
use norito::json::{self, Value};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{JsonTarget, write_json_output};

pub const DEFAULT_MAX_BUCKET_RECORDS: usize = 25;

#[derive(Clone, Debug)]
pub struct PrivacyReportOptions {
    pub input_paths: Vec<PathBuf>,
    pub bucket_config: PrivacyBucketConfig,
    pub drain_at_unix: Option<u64>,
}

pub struct PrivacyReportResult {
    pub config: PrivacyBucketConfig,
    pub sources: Vec<PathBuf>,
    pub event_count: u64,
    pub share_count: u64,
    pub bucket_count: usize,
    pub suppressed_count: usize,
    pub buckets: Vec<SoranetPrivacyBucketMetricsV1>,
    suppression: BTreeMap<SoranetPrivacySuppressionReasonV1, SuppressionStats>,
}

impl PrivacyReportResult {
    fn suppression(&self) -> &BTreeMap<SoranetPrivacySuppressionReasonV1, SuppressionStats> {
        &self.suppression
    }

    pub fn suppression_ratio(&self) -> Option<f64> {
        if self.bucket_count == 0 {
            None
        } else {
            Some(self.suppressed_count as f64 / self.bucket_count as f64)
        }
    }
}

#[derive(Default, Clone)]
pub struct SuppressionStats {
    total: usize,
    by_mode: BTreeMap<SoranetPrivacyModeV1, usize>,
}

#[derive(Default)]
struct IngestTotals {
    events: u64,
    shares: u64,
    max_bucket_idx: Option<u64>,
}

impl IngestTotals {
    fn seen_bucket(&mut self, timestamp: u64, bucket_secs: u64) {
        let idx = timestamp / bucket_secs;
        self.max_bucket_idx = Some(match self.max_bucket_idx {
            Some(existing) => existing.max(idx),
            None => idx,
        });
    }
}

pub fn run_privacy_report(
    options: &PrivacyReportOptions,
) -> Result<PrivacyReportResult, Box<dyn Error>> {
    if options.input_paths.is_empty() {
        return Err("soranet-privacy-report requires at least one --input <path>".into());
    }
    let aggregator = SoranetSecureAggregator::new(options.bucket_config)
        .map_err(|err| wrap_config_error(err, &options.bucket_config))?;
    let mut totals = IngestTotals::default();
    let bucket_secs = options.bucket_config.bucket_secs;
    for path in &options.input_paths {
        ingest_path(&aggregator, path, bucket_secs, &mut totals)?;
    }

    let drain_idx = determine_drain_index(&totals, options, bucket_secs);
    let drain_time = system_time_from_index(drain_idx, bucket_secs);
    let mut buckets = aggregator.drain_ready(drain_time);
    let suppressed_count = buckets
        .iter()
        .filter(|bucket| bucket.is_suppressed())
        .count();
    let mut suppression: BTreeMap<SoranetPrivacySuppressionReasonV1, SuppressionStats> =
        BTreeMap::new();
    for bucket in &buckets {
        if bucket.is_suppressed() {
            let reason = bucket
                .suppression_reason
                .unwrap_or(SoranetPrivacySuppressionReasonV1::InsufficientContributors);
            let entry = suppression.entry(reason).or_default();
            entry.total = entry.total.saturating_add(1);
            *entry.by_mode.entry(bucket.mode).or_insert(0) += 1;
        }
    }
    buckets.sort_by_key(|bucket| (bucket.bucket_start_unix, bucket.mode));

    Ok(PrivacyReportResult {
        config: options.bucket_config,
        sources: options.input_paths.clone(),
        event_count: totals.events,
        share_count: totals.shares,
        bucket_count: buckets.len(),
        suppressed_count,
        buckets,
        suppression,
    })
}

pub fn print_summary(result: &PrivacyReportResult, max_records: usize) {
    println!(
        "Processed {} file(s); events={} shares={}",
        result.sources.len(),
        result.event_count,
        result.share_count
    );
    println!(
        "Buckets ready: {} (suppressed {})",
        result.bucket_count, result.suppressed_count
    );
    if result.bucket_count == 0 {
        println!("No buckets ready; confirm the input files contain recent NDJSON events.");
    } else if let Some(ratio) = result.suppression_ratio() {
        println!(
            "Suppression ratio: {}/{} ({:.2}%)",
            result.suppressed_count,
            result.bucket_count,
            ratio * 100.0
        );
    }
    if result.suppressed_count == 0 {
        println!("No suppressed buckets detected.");
    } else {
        println!("Suppression breakdown:");
        for (reason, stats) in result.suppression() {
            let mut line = format!("- {}: {} bucket(s)", reason.as_label(), stats.total);
            for (mode, count) in &stats.by_mode {
                let _ = write!(line, " ({}: {})", mode.as_label(), count);
            }
            println!("{line}");
        }
    }
    let sample = build_sample(&result.buckets, max_records);
    if sample.is_empty() {
        return;
    }
    println!(
        "Sampled buckets ({} shown{}):",
        sample.len(),
        omitted_suffix(result, sample.len())
    );
    for bucket in sample {
        println!("  - {}", format_bucket(bucket));
    }
}

pub fn write_report_json(
    result: &PrivacyReportResult,
    json_out: JsonTarget,
    max_records: usize,
) -> Result<(), Box<dyn Error>> {
    let value = build_report_value(result, max_records);
    write_json_output(&value, json_out)
}

fn build_report_value(result: &PrivacyReportResult, max_records: usize) -> Value {
    let config = &result.config;
    let suppression: Vec<Value> = result
        .suppression()
        .iter()
        .map(|(reason, stats)| {
            let mut by_mode = json::Map::new();
            for (mode, count) in &stats.by_mode {
                by_mode.insert(mode.as_label().to_string(), Value::from(*count as u64));
            }
            let mut obj = json::Map::new();
            obj.insert("reason".to_string(), Value::from(reason.as_label()));
            obj.insert("count".to_string(), Value::from(stats.total as u64));
            obj.insert("by_mode".to_string(), Value::Object(by_mode));
            Value::Object(obj)
        })
        .collect();
    let suppression_ratio_value = result
        .suppression_ratio()
        .map(Value::from)
        .unwrap_or(Value::Null);
    let buckets: Vec<Value> = result
        .buckets
        .iter()
        .take(max_records)
        .map(bucket_to_value)
        .collect();

    let mut config_obj = json::Map::new();
    config_obj.insert("bucket_secs".to_string(), Value::from(config.bucket_secs));
    config_obj.insert(
        "min_contributors".to_string(),
        Value::from(config.min_contributors),
    );
    config_obj.insert(
        "flush_delay_buckets".to_string(),
        Value::from(config.flush_delay_buckets),
    );
    config_obj.insert(
        "force_flush_buckets".to_string(),
        Value::from(config.force_flush_buckets),
    );
    config_obj.insert(
        "max_completed_buckets".to_string(),
        Value::from(config.max_completed_buckets as u64),
    );
    config_obj.insert(
        "expected_shares".to_string(),
        Value::from(config.expected_shares),
    );

    let sources = result
        .sources
        .iter()
        .map(|path| Value::from(path.display().to_string()))
        .collect();

    let mut root = json::Map::new();
    root.insert("config".to_string(), Value::Object(config_obj));
    root.insert("sources".to_string(), Value::Array(sources));
    root.insert(
        "events_ingested".to_string(),
        Value::from(result.event_count),
    );
    root.insert(
        "shares_ingested".to_string(),
        Value::from(result.share_count),
    );
    root.insert(
        "bucket_count".to_string(),
        Value::from(result.bucket_count as u64),
    );
    root.insert(
        "suppressed_buckets".to_string(),
        Value::from(result.suppressed_count as u64),
    );
    root.insert("suppression_ratio".to_string(), suppression_ratio_value);
    root.insert(
        "suppression_breakdown".to_string(),
        Value::Array(suppression),
    );
    root.insert("buckets".to_string(), Value::Array(buckets));
    root.insert(
        "omitted_buckets".to_string(),
        Value::from(
            result
                .bucket_count
                .saturating_sub(result.buckets.iter().take(max_records).count()) as u64,
        ),
    );
    Value::Object(root)
}

fn bucket_to_value(bucket: &SoranetPrivacyBucketMetricsV1) -> Value {
    let iso = format_bucket_timestamp(bucket.bucket_start_unix);
    let rtt_percentiles: Vec<Value> = bucket
        .rtt_percentiles_ms
        .iter()
        .map(|entry| {
            let mut obj = json::Map::new();
            obj.insert("label".to_string(), Value::from(entry.label.clone()));
            obj.insert("value_ms".to_string(), Value::from(entry.value_ms));
            Value::Object(obj)
        })
        .collect();
    let gar_counts: Vec<Value> = bucket
        .gar_abuse_counts
        .iter()
        .map(|count| {
            let mut obj = json::Map::new();
            obj.insert(
                "category_hash_hex".to_string(),
                Value::from(hex::encode(count.category_hash)),
            );
            obj.insert("count".to_string(), Value::from(count.count));
            Value::Object(obj)
        })
        .collect();

    let mut handshake = json::Map::new();
    handshake.insert(
        "accept".to_string(),
        Value::from(bucket.handshake_accept_total),
    );
    handshake.insert(
        "pow_reject".to_string(),
        Value::from(bucket.handshake_pow_reject_total),
    );
    handshake.insert(
        "downgrade".to_string(),
        Value::from(bucket.handshake_downgrade_total),
    );
    handshake.insert(
        "timeout".to_string(),
        Value::from(bucket.handshake_timeout_total),
    );
    handshake.insert(
        "other_failure".to_string(),
        Value::from(bucket.handshake_other_failure_total),
    );

    let mut throttle = json::Map::new();
    throttle.insert(
        "congestion".to_string(),
        Value::from(bucket.throttle_congestion_total),
    );
    throttle.insert(
        "cooldown".to_string(),
        Value::from(bucket.throttle_cooldown_total),
    );
    throttle.insert(
        "emergency".to_string(),
        Value::from(bucket.throttle_emergency_total),
    );
    throttle.insert(
        "remote_quota".to_string(),
        Value::from(bucket.throttle_remote_total),
    );
    throttle.insert(
        "descriptor_quota".to_string(),
        Value::from(bucket.throttle_descriptor_total),
    );
    throttle.insert(
        "descriptor_replay".to_string(),
        Value::from(bucket.throttle_descriptor_replay_total),
    );

    let mut active = json::Map::new();
    active.insert(
        "mean".to_string(),
        bucket
            .active_circuits_mean
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    active.insert(
        "max".to_string(),
        bucket
            .active_circuits_max
            .map(Value::from)
            .unwrap_or(Value::Null),
    );

    let mut root = json::Map::new();
    root.insert("mode".to_string(), Value::from(bucket.mode.as_label()));
    root.insert(
        "bucket_start_unix".to_string(),
        Value::from(bucket.bucket_start_unix),
    );
    root.insert("bucket_start_iso".to_string(), Value::from(iso));
    root.insert(
        "bucket_duration_secs".to_string(),
        Value::from(bucket.bucket_duration_secs),
    );
    root.insert(
        "contributor_count".to_string(),
        Value::from(bucket.contributor_count),
    );
    root.insert("handshake".to_string(), Value::Object(handshake));
    root.insert("throttle".to_string(), Value::Object(throttle));
    root.insert("active_circuits".to_string(), Value::Object(active));
    let verified_bytes_value = match u64::try_from(bucket.verified_bytes_total) {
        Ok(total) => Value::from(total),
        Err(_) => Value::from(bucket.verified_bytes_total.to_string()),
    };
    root.insert("verified_bytes_total".to_string(), verified_bytes_value);
    root.insert(
        "rtt_percentiles_ms".to_string(),
        Value::Array(rtt_percentiles),
    );
    root.insert("gar_abuse_counts".to_string(), Value::Array(gar_counts));
    root.insert("suppressed".to_string(), Value::from(bucket.suppressed));
    root.insert(
        "suppression_reason".to_string(),
        bucket
            .suppression_reason
            .map(SoranetPrivacySuppressionReasonV1::as_label)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    Value::Object(root)
}

fn build_sample(
    buckets: &[SoranetPrivacyBucketMetricsV1],
    max_records: usize,
) -> Vec<&SoranetPrivacyBucketMetricsV1> {
    if buckets.is_empty() || max_records == 0 {
        return Vec::new();
    }
    let suppressed: Vec<_> = buckets
        .iter()
        .filter(|bucket| bucket.is_suppressed())
        .take(max_records)
        .collect();
    if suppressed.is_empty() {
        buckets.iter().take(max_records).collect()
    } else {
        suppressed
    }
}

fn omitted_suffix(result: &PrivacyReportResult, shown: usize) -> String {
    let omitted = result.bucket_count.saturating_sub(shown);
    if omitted == 0 {
        String::new()
    } else {
        format!(", {} omitted", omitted)
    }
}

fn format_bucket(bucket: &SoranetPrivacyBucketMetricsV1) -> String {
    let timestamp = format_bucket_timestamp(bucket.bucket_start_unix);
    if bucket.is_suppressed() {
        let reason = bucket
            .suppression_reason
            .map(|reason| reason.as_label())
            .unwrap_or("insufficient_contributors");
        format!(
            "{timestamp} mode={} suppressed ({reason})",
            bucket.mode.as_label()
        )
    } else {
        format!(
            "{timestamp} mode={} contributors={} accept={} throttles={}",
            bucket.mode.as_label(),
            bucket.contributor_count,
            bucket.handshake_accept_total,
            bucket
                .throttle_congestion_total
                .saturating_add(bucket.throttle_cooldown_total)
                .saturating_add(bucket.throttle_emergency_total)
                .saturating_add(bucket.throttle_remote_total)
                .saturating_add(bucket.throttle_descriptor_total)
                .saturating_add(bucket.throttle_descriptor_replay_total)
        )
    }
}

fn format_bucket_timestamp(bucket_start_unix: u64) -> String {
    OffsetDateTime::from_unix_timestamp(bucket_start_unix as i64)
        .map(|dt| {
            dt.format(&Rfc3339)
                .unwrap_or_else(|_| bucket_start_unix.to_string())
        })
        .unwrap_or_else(|_| bucket_start_unix.to_string())
}

fn ingest_path(
    aggregator: &SoranetSecureAggregator,
    path: &Path,
    bucket_secs: u64,
    totals: &mut IngestTotals,
) -> Result<(), Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    for (idx, line) in reader.lines().enumerate() {
        let raw = line?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match json::from_str::<SoranetPrivacyEventV1>(trimmed) {
            Ok(event) => {
                aggregator.record_event(&event);
                totals.events = totals.events.saturating_add(1);
                totals.seen_bucket(event.timestamp_unix, bucket_secs);
            }
            Err(event_error) => match json::from_str::<SoranetPrivacyPrioShareV1>(trimmed) {
                Ok(share) => {
                    let bucket_start_unix = share.bucket_start_unix;
                    aggregator.ingest_prio_share(share).map_err(|err| {
                        format!(
                            "failed to ingest privacy share from {} line {}: {err}",
                            path.display(),
                            idx + 1
                        )
                    })?;
                    totals.shares = totals.shares.saturating_add(1);
                    totals.seen_bucket(bucket_start_unix, bucket_secs);
                }
                Err(share_error) => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
                        && let Some(event) = parse_fallback_event(&value)
                    {
                        aggregator.record_event(&event);
                        totals.events = totals.events.saturating_add(1);
                        totals.seen_bucket(event.timestamp_unix, bucket_secs);
                        continue;
                    }
                    return Err(format!(
                        "failed to parse privacy payload in {} line {}: {event_error}; {share_error}",
                        path.display(),
                        idx + 1
                    )
                    .into());
                }
            },
        }
    }
    Ok(())
}

fn parse_fallback_event(value: &serde_json::Value) -> Option<SoranetPrivacyEventV1> {
    let timestamp_unix = value.get("timestamp_unix")?.as_u64()?;
    let mode = match value.get("mode")?.as_str()? {
        "entry" => SoranetPrivacyModeV1::Entry,
        "middle" => SoranetPrivacyModeV1::Middle,
        "exit" => SoranetPrivacyModeV1::Exit,
        _ => return None,
    };
    let payload = value.get("payload").cloned().unwrap_or_default();
    let kind = match value.get("kind")?.as_str()? {
        "HandshakeSuccess" => {
            let rtt_ms = payload.get("rtt_ms").and_then(|v| v.as_u64());
            let active = payload
                .get("active_circuits_after")
                .and_then(|v| v.as_u64());
            SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
                rtt_ms,
                active_circuits_after: active,
            })
        }
        "HandshakeFailure" => {
            let reason_slug = payload
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("other");
            let reason = match reason_slug {
                "pow" => SoranetPrivacyHandshakeFailureV1::Pow,
                "timeout" => SoranetPrivacyHandshakeFailureV1::Timeout,
                "downgrade" => SoranetPrivacyHandshakeFailureV1::Downgrade,
                _ => SoranetPrivacyHandshakeFailureV1::Other,
            };
            let detail = payload
                .get("detail")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let rtt_ms = payload.get("rtt_ms").and_then(|v| v.as_u64());
            SoranetPrivacyEventKindV1::HandshakeFailure(SoranetPrivacyEventHandshakeFailureV1 {
                reason,
                detail,
                rtt_ms,
            })
        }
        _ => return None,
    };
    Some(SoranetPrivacyEventV1 {
        timestamp_unix,
        mode,
        kind,
    })
}

fn determine_drain_index(
    totals: &IngestTotals,
    options: &PrivacyReportOptions,
    bucket_secs: u64,
) -> u64 {
    let extra = options
        .bucket_config
        .force_flush_buckets
        .max(options.bucket_config.flush_delay_buckets)
        .saturating_add(1);
    if let Some(explicit) = options.drain_at_unix {
        return explicit / bucket_secs;
    }
    if let Some(idx) = totals.max_bucket_idx {
        idx.saturating_add(extra)
    } else {
        current_unix_index(bucket_secs)
    }
}

fn system_time_from_index(idx: u64, bucket_secs: u64) -> SystemTime {
    let secs = idx.saturating_mul(bucket_secs);
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn current_unix_index(bucket_secs: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    now.as_secs() / bucket_secs.max(1)
}

fn wrap_config_error(err: PrivacyConfigError, config: &PrivacyBucketConfig) -> Box<dyn Error> {
    format!(
        "invalid privacy bucket config (bucket_secs={}, min_contributors={}): {err}",
        config.bucket_secs, config.min_contributors
    )
    .into()
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use tempfile::NamedTempFile;

    use super::*;

    fn write_log(lines: &[&str]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp file");
        for line in lines {
            writeln!(file, "{}", line).expect("write log");
        }
        file
    }

    fn base_config() -> PrivacyBucketConfig {
        PrivacyBucketConfig {
            bucket_secs: 60,
            min_contributors: 1,
            flush_delay_buckets: 0,
            force_flush_buckets: 1,
            max_completed_buckets: 64,
            expected_shares: 1,
            max_share_lag_buckets: 12,
        }
    }

    #[test]
    fn summarizes_unsuppressed_bucket() {
        let log = write_log(&[
            "{\"timestamp_unix\":100,\"mode\":\"entry\",\"kind\":\"HandshakeSuccess\",\"payload\":{\"rtt_ms\":12}}",
        ]);
        let options = PrivacyReportOptions {
            input_paths: vec![log.path().to_path_buf()],
            bucket_config: base_config(),
            drain_at_unix: Some(200),
        };
        let report = run_privacy_report(&options).expect("report");
        assert_eq!(report.event_count, 1);
        assert_eq!(report.share_count, 0);
        assert_eq!(report.bucket_count, 1);
        assert_eq!(report.suppressed_count, 0);
        assert_eq!(report.suppression_ratio(), Some(0.0));
        let json = build_report_value(&report, 1);
        assert_eq!(json["bucket_count"], norito::json!(1u64));
        assert_eq!(json["buckets"].as_array().unwrap().len(), 1);
        assert_eq!(json["suppression_ratio"], norito::json!(0.0));
    }

    #[test]
    fn reports_suppressed_bucket_breakdown() {
        let log = write_log(&[
            "{\"timestamp_unix\":3600,\"mode\":\"exit\",\"kind\":\"HandshakeFailure\",\"payload\":{\"reason\":\"timeout\"}}",
        ]);
        let mut config = base_config();
        config.min_contributors = 5;
        let options = PrivacyReportOptions {
            input_paths: vec![log.path().to_path_buf()],
            bucket_config: config,
            drain_at_unix: Some(7200),
        };
        let report = run_privacy_report(&options).expect("report");
        assert_eq!(report.bucket_count, 1);
        assert_eq!(report.suppressed_count, 1);
        assert_eq!(report.suppression_ratio(), Some(1.0));
        let breakdown = report.suppression();
        assert_eq!(breakdown.len(), 1);
        let stats = breakdown
            .get(&SoranetPrivacySuppressionReasonV1::ForcedFlushWindowElapsed)
            .or(breakdown.get(&SoranetPrivacySuppressionReasonV1::InsufficientContributors))
            .expect("suppression entry");
        assert_eq!(stats.total, 1);
        assert_eq!(stats.by_mode.get(&SoranetPrivacyModeV1::Exit), Some(&1));
    }

    #[test]
    fn suppression_ratio_reports_fraction() {
        let mut events: Vec<&'static str> = vec![
            "{\"timestamp_unix\":0,\"mode\":\"entry\",\"kind\":\"HandshakeSuccess\",\"payload\":{\"rtt_ms\":9}}";
            3
        ];
        events.push(
            "{\"timestamp_unix\":180,\"mode\":\"entry\",\"kind\":\"HandshakeSuccess\",\"payload\":{\"rtt_ms\":11}}",
        );
        let log = write_log(&events);
        let mut config = base_config();
        config.min_contributors = 2;
        let options = PrivacyReportOptions {
            input_paths: vec![log.path().to_path_buf()],
            bucket_config: config,
            drain_at_unix: Some(600),
        };
        let report = run_privacy_report(&options).expect("report");
        assert_eq!(report.bucket_count, 2);
        assert_eq!(report.suppressed_count, 1);
        assert_eq!(report.suppression_ratio(), Some(0.5));
    }
}
