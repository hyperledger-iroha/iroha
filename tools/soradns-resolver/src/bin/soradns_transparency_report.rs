use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Write as _,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use clap::Parser;
use eyre::{Result, WrapErr};
use norito::json::{self, Value};
use soradns_resolver::transparency::{
    BundleEventKind, BundleRecord, BundleState, ResolverEventKind, ResolverRecord,
    TransparencyRecord, TransparencyTailer,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const DEFAULT_RECENT_LIMIT: usize = 20;

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Summarise resolver transparency logs with Signed Tree Head digests."
)]
struct Cli {
    /// Path to the resolver transparency log (defaults to stdin).
    #[arg(long)]
    log: Option<PathBuf>,
    /// Destination for the rendered report (defaults to stdout).
    #[arg(long)]
    output: Option<PathBuf>,
    /// Optional JSON file describing the latest Signed Tree Heads.
    #[arg(long)]
    sth_output: Option<PathBuf>,
    /// Maximum number of recent events included in the report.
    #[arg(long, default_value_t = DEFAULT_RECENT_LIMIT)]
    recent_limit: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut tailer = TransparencyTailer::new();
    let mut builder = ReportBuilder::new(cli.recent_limit);
    ingest_log(cli.log.as_ref(), &mut tailer, &mut builder)?;

    let generated_at = OffsetDateTime::now_utc();
    let report_body = render_report(&tailer, &builder, generated_at);
    write_report_output(cli.output.as_ref(), &report_body)?;

    if let Some(path) = cli.sth_output.as_ref() {
        write_sth_summary(path, &tailer, generated_at)?;
    }

    Ok(())
}

fn ingest_log(
    log_path: Option<&PathBuf>,
    tailer: &mut TransparencyTailer,
    builder: &mut ReportBuilder,
) -> Result<()> {
    let reader: Box<dyn BufRead> = match log_path {
        Some(path) => {
            let file = File::open(path).wrap_err("failed to open transparency log")?;
            Box::new(BufReader::new(file))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    let mut line = String::new();
    let mut reader = reader;
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .wrap_err("failed reading transparency line")?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match tailer.ingest_line(trimmed) {
            Ok(records) => {
                for record in records {
                    builder.record(&record);
                }
            }
            Err(error) => {
                eprintln!("transparency report warning: {error}");
            }
        }
    }
    Ok(())
}

fn write_report_output(path: Option<&PathBuf>, contents: &str) -> Result<()> {
    match path {
        Some(path) => {
            let mut writer =
                BufWriter::new(File::create(path).wrap_err("failed to create report output file")?);
            writer
                .write_all(contents.as_bytes())
                .wrap_err("failed to write report output")?
        }
        None => {
            let mut writer = BufWriter::new(io::stdout());
            writer
                .write_all(contents.as_bytes())
                .wrap_err("failed to write report output")?;
            writer.flush().wrap_err("failed to flush stdout")?;
        }
    }
    Ok(())
}

fn render_report(
    tailer: &TransparencyTailer,
    builder: &ReportBuilder,
    generated_at: OffsetDateTime,
) -> String {
    let states = tailer.bundle_states();
    let resolver_summaries = build_resolver_summaries(&states);
    let recent_events: Vec<_> = builder.recent_events.iter().rev().cloned().collect();
    let generated_fmt = format_timestamp(generated_at.unix_timestamp());
    let mut report = String::new();
    writeln!(report, "# SoraDNS Transparency Report").ok();
    writeln!(report, "Generated at: {}", generated_fmt).ok();
    writeln!(report).ok();

    render_summary_section(&mut report, &states, builder);
    render_resolver_table(&mut report, &resolver_summaries);
    render_sth_table(&mut report, &states);
    render_recent_events(&mut report, &recent_events);
    report
}

fn render_summary_section(report: &mut String, states: &[BundleState], builder: &ReportBuilder) {
    let unique_resolvers: HashSet<_> = states
        .iter()
        .map(|state| state.resolver_id.clone())
        .collect();
    let max_age = states.iter().max_by_key(|state| state.proof_age_secs);
    let min_ttl = states.iter().min_by_key(|state| state.proof_ttl_secs);

    writeln!(report, "## Summary").ok();
    writeln!(
        report,
        "- Total events processed: {} (added: {}, updated: {}, reorged: {}, removed: {}; resolver updates: {}/{}/{}).",
        builder.counters.total,
        builder.counters.bundle_added,
        builder.counters.bundle_updated,
        builder.counters.bundle_reorged,
        builder.counters.bundle_removed,
        builder.counters.resolver_added,
        builder.counters.resolver_updated,
        builder.counters.resolver_removed
    )
    .ok();
    writeln!(
        report,
        "- Active resolvers: {} covering {} bundle snapshots.",
        unique_resolvers.len(),
        states.len()
    )
    .ok();
    if let Some(state) = max_age {
        writeln!(
            report,
            "- Max proof age: {} s ({} / {}).",
            state.proof_age_secs, state.resolver_id, state.namehash
        )
        .ok();
    } else {
        writeln!(report, "- Max proof age: n/a").ok();
    }
    if let Some(state) = min_ttl {
        writeln!(
            report,
            "- Shortest proof TTL: {} s ({} / {}).",
            state.proof_ttl_secs, state.resolver_id, state.namehash
        )
        .ok();
    } else {
        writeln!(report, "- Shortest proof TTL: n/a").ok();
    }
    writeln!(
        report,
        "- Freeze events recorded: {} — Reorg events detected: {}.",
        builder.counters.bundle_removed, builder.counters.bundle_reorged
    )
    .ok();
    writeln!(report).ok();
}

fn render_resolver_table(report: &mut String, summaries: &[ResolverSummary]) {
    writeln!(report, "## Resolver Health").ok();
    writeln!(
        report,
        "| Resolver | Active Zones | Max Proof Age (s) | Min Proof TTL (s) | CID Drift |"
    )
    .ok();
    writeln!(report, "| --- | ---: | ---: | ---: | ---: |").ok();
    for summary in summaries {
        writeln!(
            report,
            "| {} | {} | {} | {} | {} |",
            summary.resolver_id,
            summary.active_zones,
            summary.max_proof_age_secs,
            format_optional_i64(summary.min_proof_ttl_secs),
            summary.cid_drift_total
        )
        .ok();
    }
    writeln!(report).ok();
}

fn render_sth_table(report: &mut String, states: &[BundleState]) {
    writeln!(report, "## Signed Tree Heads").ok();
    writeln!(
        report,
        "| Resolver | Namehash | Zone Version | Policy Hash | Manifest Hash | CAR Root CID | Expires At (UTC) | Signer |"
    )
    .ok();
    writeln!(report, "| --- | --- | ---: | --- | --- | --- | --- | --- |").ok();
    for state in states {
        let snapshot = &state.snapshot;
        writeln!(
            report,
            "| {} | {} | {} | `{}` | `{}` | `{}` | {} | `{}` |",
            state.resolver_id,
            state.namehash,
            snapshot.zone_version,
            snapshot.policy_hash_hex,
            snapshot.manifest_hash_hex,
            snapshot.car_root_cid,
            format_timestamp(snapshot.freshness_expires_at as i64),
            snapshot.freshness_signer
        )
        .ok();
    }
    writeln!(report).ok();
}

fn render_recent_events(report: &mut String, events: &[TimelineEvent]) {
    writeln!(report, "## Recent Events").ok();
    if events.is_empty() {
        writeln!(report, "No recent transparency events recorded.").ok();
        return;
    }
    writeln!(
        report,
        "| Timestamp | Resolver | Namehash | Event | Zone | CID Drift |"
    )
    .ok();
    writeln!(report, "| --- | --- | --- | --- | ---: | --- |").ok();
    for event in events {
        let namehash = event
            .namehash
            .as_deref()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".into());
        let zone = event
            .zone_version
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".into());
        let cid = event
            .cid_changed
            .map(|changed| if changed { "yes" } else { "no" })
            .unwrap_or("-");
        writeln!(
            report,
            "| {} | {} | {} | {} | {} | {} |",
            format_timestamp(event.timestamp),
            event.resolver_id,
            namehash,
            event.label,
            zone,
            cid
        )
        .ok();
    }
    writeln!(report).ok();
}

fn build_resolver_summaries(states: &[BundleState]) -> Vec<ResolverSummary> {
    let mut summaries: HashMap<String, ResolverSummary> = HashMap::new();
    for state in states {
        let entry = summaries
            .entry(state.resolver_id.clone())
            .or_insert_with(|| ResolverSummary::new(state.resolver_id.clone()));
        entry.update(state);
    }
    let mut values: Vec<_> = summaries.into_values().collect();
    values.sort_by(|a, b| a.resolver_id.cmp(&b.resolver_id));
    values
}

fn write_sth_summary(
    path: &Path,
    tailer: &TransparencyTailer,
    generated_at: OffsetDateTime,
) -> Result<()> {
    let states = tailer.bundle_states();
    let mut top = json::Map::new();
    top.insert(
        "generated_at_ms".into(),
        Value::from(unix_timestamp_ms(generated_at)),
    );
    let mut entries = Vec::with_capacity(states.len());
    for state in states {
        let mut item = json::Map::new();
        item.insert("resolver_id".into(), Value::from(state.resolver_id));
        item.insert("namehash".into(), Value::from(state.namehash));
        item.insert(
            "zone_version".into(),
            Value::from(state.snapshot.zone_version),
        );
        item.insert(
            "policy_hash_hex".into(),
            Value::from(state.snapshot.policy_hash_hex.clone()),
        );
        item.insert(
            "manifest_hash_hex".into(),
            Value::from(state.snapshot.manifest_hash_hex.clone()),
        );
        item.insert(
            "car_root_cid".into(),
            Value::from(state.snapshot.car_root_cid.clone()),
        );
        item.insert(
            "freshness_issued_at".into(),
            Value::from(state.snapshot.freshness_issued_at),
        );
        item.insert(
            "freshness_expires_at".into(),
            Value::from(state.snapshot.freshness_expires_at),
        );
        item.insert(
            "freshness_signer".into(),
            Value::from(state.snapshot.freshness_signer.clone()),
        );
        item.insert(
            "freshness_signature_hex".into(),
            Value::from(state.snapshot.freshness_signature_hex.clone()),
        );
        item.insert("cid_drift_total".into(), Value::from(state.cid_drift_total));
        entries.push(Value::Object(item));
    }
    top.insert("entries".into(), Value::Array(entries));
    let text = json::to_string_pretty(&Value::Object(top))
        .wrap_err("failed to serialise Signed Tree Head summary")?;
    let mut writer = BufWriter::new(
        File::create(path).wrap_err("failed to create Signed Tree Head summary file")?,
    );
    writer
        .write_all(text.as_bytes())
        .wrap_err("failed to write Signed Tree Head summary")?;
    writer
        .flush()
        .wrap_err("failed to flush Signed Tree Head summary")
}

fn unix_timestamp_ms(dt: OffsetDateTime) -> u64 {
    let secs = dt.unix_timestamp().max(0) as i128;
    let millis = secs * 1_000 + i128::from(dt.nanosecond() / 1_000_000);
    millis as u64
}

fn format_timestamp(ts: i64) -> String {
    match OffsetDateTime::from_unix_timestamp(ts) {
        Ok(dt) => dt.format(&Rfc3339).unwrap_or_else(|_| ts.to_string()),
        Err(_) => ts.to_string(),
    }
}

fn format_optional_i64(value: i64) -> String {
    if value == i64::MAX {
        "-".into()
    } else {
        value.to_string()
    }
}

#[derive(Default, Debug)]
struct EventCounters {
    total: u64,
    bundle_added: u64,
    bundle_updated: u64,
    bundle_reorged: u64,
    bundle_removed: u64,
    resolver_added: u64,
    resolver_updated: u64,
    resolver_removed: u64,
}

#[derive(Clone, Debug)]
struct TimelineEvent {
    timestamp: i64,
    resolver_id: String,
    namehash: Option<String>,
    label: String,
    zone_version: Option<u64>,
    cid_changed: Option<bool>,
}

struct ReportBuilder {
    counters: EventCounters,
    recent_events: VecDeque<TimelineEvent>,
    recent_limit: usize,
}

impl ReportBuilder {
    fn new(recent_limit: usize) -> Self {
        Self {
            counters: EventCounters::default(),
            recent_events: VecDeque::with_capacity(recent_limit.max(1)),
            recent_limit: recent_limit.max(1),
        }
    }

    fn record(&mut self, record: &TransparencyRecord) {
        self.counters.total = self.counters.total.saturating_add(1);
        match record {
            TransparencyRecord::Bundle(bundle) => {
                self.record_bundle(bundle);
            }
            TransparencyRecord::Resolver(resolver) => {
                self.record_resolver(resolver);
            }
        }
    }

    fn record_bundle(&mut self, bundle: &BundleRecord) {
        match bundle.event {
            BundleEventKind::Added => self.counters.bundle_added += 1,
            BundleEventKind::Updated => self.counters.bundle_updated += 1,
            BundleEventKind::Reorged => self.counters.bundle_reorged += 1,
            BundleEventKind::Removed | BundleEventKind::Expired => {
                self.counters.bundle_removed += 1
            }
        }
        self.push_event(TimelineEvent {
            timestamp: bundle.timestamp,
            resolver_id: bundle.resolver_id.clone(),
            namehash: Some(bundle.namehash.clone()),
            label: bundle.event.as_str().replace('_', " "),
            zone_version: Some(bundle.zone_version),
            cid_changed: Some(bundle.cid_changed),
        });
    }

    fn record_resolver(&mut self, record: &ResolverRecord) {
        match record.event {
            ResolverEventKind::Added => self.counters.resolver_added += 1,
            ResolverEventKind::Updated => self.counters.resolver_updated += 1,
            ResolverEventKind::Removed | ResolverEventKind::Invalidated => {
                self.counters.resolver_removed += 1
            }
        }
        self.push_event(TimelineEvent {
            timestamp: record.timestamp,
            resolver_id: record.resolver_id.clone(),
            namehash: None,
            label: record.event.as_str().replace('_', " "),
            zone_version: None,
            cid_changed: None,
        });
    }

    fn push_event(&mut self, event: TimelineEvent) {
        if self.recent_events.len() == self.recent_limit {
            self.recent_events.pop_front();
        }
        self.recent_events.push_back(event);
    }
}

#[derive(Debug, Clone)]
struct ResolverSummary {
    resolver_id: String,
    active_zones: usize,
    max_proof_age_secs: i64,
    min_proof_ttl_secs: i64,
    cid_drift_total: u64,
}

impl ResolverSummary {
    fn new(resolver_id: String) -> Self {
        Self {
            resolver_id,
            active_zones: 0,
            max_proof_age_secs: 0,
            min_proof_ttl_secs: i64::MAX,
            cid_drift_total: 0,
        }
    }

    fn update(&mut self, state: &BundleState) {
        self.active_zones += 1;
        self.max_proof_age_secs = self.max_proof_age_secs.max(state.proof_age_secs);
        self.min_proof_ttl_secs = self.min_proof_ttl_secs.min(state.proof_ttl_secs);
        self.cid_drift_total = self.cid_drift_total.saturating_add(state.cid_drift_total);
    }
}

#[cfg(test)]
mod tests {
    use norito::json::{self, Value};
    use soradns_resolver::transparency::{BundleSnapshot, TransparencyTailer};

    use super::*;

    fn sample_snapshot(zone_version: u64, cid: &str) -> BundleSnapshot {
        BundleSnapshot {
            zone_version,
            manifest_hash_hex: "bb".into(),
            policy_hash_hex: "aa".into(),
            car_root_cid: cid.into(),
            freshness_issued_at: 10,
            freshness_expires_at: 70,
            freshness_signer: "council".into(),
            freshness_signature_hex: "ff".into(),
        }
    }

    #[test]
    fn builder_tracks_recent_events() {
        let mut builder = ReportBuilder::new(2);
        builder.record(&TransparencyRecord::Bundle(Box::new(BundleRecord::new(
            20,
            "resolver-a".into(),
            "beef".into(),
            &sample_snapshot(4, "cid"),
            BundleEventKind::Added,
            None,
        ))));
        builder.record(&TransparencyRecord::Resolver(ResolverRecord {
            timestamp: 30,
            resolver_id: "resolver-a".into(),
            event: ResolverEventKind::Updated,
        }));
        builder.record(&TransparencyRecord::Bundle(Box::new(BundleRecord::new(
            40,
            "resolver-b".into(),
            "cafe".into(),
            &sample_snapshot(5, "cid2"),
            BundleEventKind::Updated,
            None,
        ))));

        assert_eq!(builder.counters.total, 3);
        assert_eq!(builder.counters.bundle_added, 1);
        assert_eq!(builder.counters.bundle_updated, 1);
        assert_eq!(builder.recent_events.len(), 2);
        assert_eq!(builder.recent_events.front().unwrap().timestamp, 30);
        assert_eq!(builder.recent_events.back().unwrap().timestamp, 40);
    }

    #[test]
    fn resolver_summary_accumulates_stats() {
        let states = vec![
            BundleState {
                resolver_id: "resolver-a".into(),
                namehash: "beef".into(),
                snapshot: sample_snapshot(7, "cid"),
                last_timestamp: 50,
                proof_age_secs: 25,
                proof_ttl_secs: 45,
                cid_drift_total: 1,
            },
            BundleState {
                resolver_id: "resolver-a".into(),
                namehash: "cafe".into(),
                snapshot: sample_snapshot(3, "cid2"),
                last_timestamp: 60,
                proof_age_secs: 30,
                proof_ttl_secs: 40,
                cid_drift_total: 2,
            },
        ];
        let summaries = build_resolver_summaries(&states);
        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];
        assert_eq!(summary.resolver_id, "resolver-a");
        assert_eq!(summary.active_zones, 2);
        assert_eq!(summary.max_proof_age_secs, 30);
        assert_eq!(summary.min_proof_ttl_secs, 40);
        assert_eq!(summary.cid_drift_total, 3);
    }

    #[test]
    fn sth_writer_serialises_entries() {
        let mut tailer = TransparencyTailer::new();
        use soradns_resolver::events::{ResolverEvent, ResolverEventLog};

        let log = ResolverEventLog {
            timestamp: 30,
            resolver_id: "resolver-a".into(),
            event: ResolverEvent::BundleAdded {
                namehash: "beef".into(),
                snapshot: sample_snapshot(2, "cid"),
            },
        };
        let serialized = json::to_value(&log)
            .and_then(|value| json::to_string(&value))
            .expect("serialize log");
        tailer.ingest_line(&serialized).expect("ingest log");
        let path = tempfile::NamedTempFile::new().expect("temp file");
        write_sth_summary(path.path(), &tailer, OffsetDateTime::now_utc()).expect("write sth");
        let contents = std::fs::read_to_string(path).expect("read sth");
        let parsed: Value = norito::json::from_str(&contents).expect("parse json");
        let entries = parsed["entries"].as_array().expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["resolver_id"].as_str(), Some("resolver-a"));
    }
}
