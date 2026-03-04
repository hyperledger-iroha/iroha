use std::{cmp::Ordering, collections::HashMap};

use norito::json::{self, Value};
use thiserror::Error;

use crate::events::{ResolverEvent, ResolverEventLog};
pub use crate::state::BundleSnapshot;

/// Structured records generated from resolver transparency logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransparencyRecord {
    Bundle(Box<BundleRecord>),
    Resolver(ResolverRecord),
}

impl TransparencyRecord {
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        match self {
            Self::Bundle(record) => record.as_ref().to_json_value(),
            Self::Resolver(record) => record.to_json_value(),
        }
    }

    #[must_use]
    pub fn to_tsv_row(&self) -> String {
        match self {
            Self::Bundle(record) => record.as_ref().to_tsv_row(),
            Self::Resolver(record) => record.to_tsv_row(),
        }
    }
}

/// Bundle-specific transparency metrics captured from resolver events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleRecord {
    pub timestamp: i64,
    pub resolver_id: String,
    pub namehash: String,
    pub snapshot: BundleSnapshot,
    pub zone_version: u64,
    pub previous_zone_version: Option<u64>,
    pub policy_hash_hex: String,
    pub manifest_hash_hex: String,
    pub car_root_cid: String,
    pub proof_age_secs: i64,
    pub proof_ttl_secs: i64,
    pub cid_changed: bool,
    pub event: BundleEventKind,
    pub freshness_signer: String,
    pub freshness_signature_hex: String,
}

/// Current bundle snapshot plus derived telemetry for report generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleState {
    pub resolver_id: String,
    pub namehash: String,
    pub snapshot: BundleSnapshot,
    pub last_timestamp: i64,
    pub proof_age_secs: i64,
    pub proof_ttl_secs: i64,
    pub cid_drift_total: u64,
}

impl BundleRecord {
    #[must_use]
    pub fn new(
        timestamp: i64,
        resolver_id: String,
        namehash: String,
        snapshot: &BundleSnapshot,
        event: BundleEventKind,
        previous: Option<&BundleSnapshot>,
    ) -> Self {
        let proof_age_secs = timestamp.saturating_sub(snapshot.freshness_issued_at as i64);
        let proof_ttl_secs = (snapshot.freshness_expires_at as i64).saturating_sub(timestamp);
        let previous_zone_version = previous.map(|p| p.zone_version);
        let cid_changed = previous
            .map(|p| p.car_root_cid != snapshot.car_root_cid)
            .unwrap_or(false);

        Self {
            timestamp,
            resolver_id,
            namehash,
            snapshot: snapshot.clone(),
            zone_version: snapshot.zone_version,
            previous_zone_version,
            policy_hash_hex: snapshot.policy_hash_hex.clone(),
            manifest_hash_hex: snapshot.manifest_hash_hex.clone(),
            car_root_cid: snapshot.car_root_cid.clone(),
            proof_age_secs,
            proof_ttl_secs,
            cid_changed,
            event,
            freshness_signer: snapshot.freshness_signer.clone(),
            freshness_signature_hex: snapshot.freshness_signature_hex.clone(),
        }
    }

    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let mut map = json::Map::new();
        map.insert("record_type".into(), Value::from("bundle"));
        map.insert("timestamp".into(), Value::from(self.timestamp));
        map.insert("resolver_id".into(), Value::from(self.resolver_id.clone()));
        map.insert("namehash".into(), Value::from(self.namehash.clone()));
        map.insert("zone_version".into(), Value::from(self.zone_version));
        map.insert(
            "previous_zone_version".into(),
            self.previous_zone_version.map_or(Value::Null, Value::from),
        );
        map.insert(
            "policy_hash_hex".into(),
            Value::from(self.policy_hash_hex.clone()),
        );
        map.insert(
            "manifest_hash_hex".into(),
            Value::from(self.manifest_hash_hex.clone()),
        );
        map.insert(
            "car_root_cid".into(),
            Value::from(self.car_root_cid.clone()),
        );
        map.insert("proof_age_secs".into(), Value::from(self.proof_age_secs));
        map.insert("proof_ttl_secs".into(), Value::from(self.proof_ttl_secs));
        map.insert("cid_changed".into(), Value::from(self.cid_changed));
        map.insert("event".into(), Value::from(self.event.as_str()));
        map.insert(
            "freshness_signer".into(),
            Value::from(self.freshness_signer.clone()),
        );
        map.insert(
            "freshness_signature_hex".into(),
            Value::from(self.freshness_signature_hex.clone()),
        );
        Value::Object(map)
    }

    #[must_use]
    pub fn to_tsv_row(&self) -> String {
        format!(
            "bundle\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t-",
            self.timestamp,
            self.resolver_id,
            self.namehash,
            self.zone_version,
            self.previous_zone_version
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into()),
            self.policy_hash_hex,
            self.manifest_hash_hex,
            self.car_root_cid,
            self.proof_age_secs,
            self.proof_ttl_secs,
            if self.cid_changed { "1" } else { "0" },
            self.event.as_str(),
            self.freshness_signer,
            self.freshness_signature_hex
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleEventKind {
    Added,
    Updated,
    Reorged,
    Removed,
    Expired,
}

impl BundleEventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "bundle_added",
            Self::Updated => "bundle_updated",
            Self::Reorged => "bundle_reorged",
            Self::Removed => "bundle_removed",
            Self::Expired => "bundle_expired",
        }
    }
}

#[derive(Debug, Clone)]
struct BundleMetricState {
    snapshot: BundleSnapshot,
    last_timestamp: i64,
    proof_age_secs: i64,
    proof_ttl_secs: i64,
    cid_drift_total: u64,
}

impl BundleMetricState {
    fn from_snapshot(snapshot: &BundleSnapshot, timestamp: i64) -> Self {
        Self {
            snapshot: snapshot.clone(),
            last_timestamp: timestamp,
            proof_age_secs: timestamp.saturating_sub(snapshot.freshness_issued_at as i64),
            proof_ttl_secs: (snapshot.freshness_expires_at as i64).saturating_sub(timestamp),
            cid_drift_total: 0,
        }
    }

    fn update(&mut self, snapshot: &BundleSnapshot, timestamp: i64, cid_changed: bool) {
        self.snapshot = snapshot.clone();
        self.last_timestamp = timestamp;
        self.proof_age_secs = timestamp.saturating_sub(snapshot.freshness_issued_at as i64);
        self.proof_ttl_secs = (snapshot.freshness_expires_at as i64).saturating_sub(timestamp);
        if cid_changed {
            self.cid_drift_total = self.cid_drift_total.saturating_add(1);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricLine {
    pub metric: &'static str,
    pub labels: Vec<(&'static str, String)>,
    pub value: f64,
}

impl MetricLine {
    #[must_use]
    pub fn to_prometheus(&self) -> String {
        let mut rendered = String::new();
        rendered.push_str(self.metric);
        if !self.labels.is_empty() {
            rendered.push('{');
            for (idx, (key, value)) in self.labels.iter().enumerate() {
                if idx > 0 {
                    rendered.push(',');
                }
                rendered.push_str(key);
                rendered.push('=');
                rendered.push('"');
                for ch in value.chars() {
                    match ch {
                        '"' => rendered.push_str("\\\""),
                        '\\' => rendered.push_str("\\\\"),
                        '\n' => rendered.push_str("\\n"),
                        other => rendered.push(other),
                    }
                }
                rendered.push('"');
            }
            rendered.push('}');
        }
        rendered.push(' ');
        rendered.push_str(&format!("{}", self.value));
        rendered
    }
}

/// Resolver-advert transparency record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverRecord {
    pub timestamp: i64,
    pub resolver_id: String,
    pub event: ResolverEventKind,
}

impl ResolverRecord {
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let mut map = json::Map::new();
        map.insert("record_type".into(), Value::from("resolver"));
        map.insert("timestamp".into(), Value::from(self.timestamp));
        map.insert("resolver_id".into(), Value::from(self.resolver_id.clone()));
        map.insert("event".into(), Value::from(self.event.as_str()));
        Value::Object(map)
    }

    #[must_use]
    pub fn to_tsv_row(&self) -> String {
        format!(
            "resolver\t{}\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t{}\t{}",
            self.timestamp,
            self.resolver_id,
            self.event.as_str()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverEventKind {
    Added,
    Updated,
    Removed,
    Invalidated,
}

impl ResolverEventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "resolver_added",
            Self::Updated => "resolver_updated",
            Self::Removed => "resolver_removed",
            Self::Invalidated => "resolver_invalidated",
        }
    }
}

/// Error raised while parsing transparency log lines.
#[derive(Debug, Error)]
pub enum TransparencyError {
    #[error("failed to parse resolver log line: {0}")]
    Parse(#[from] norito::json::Error),
}

/// Stateful tailer that converts resolver log lines into structured transparency records.
#[derive(Debug, Default)]
pub struct TransparencyTailer {
    bundles: HashMap<(String, String), BundleSnapshot>,
    bundle_metrics: HashMap<(String, String), BundleMetricState>,
}

impl TransparencyTailer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bundles: HashMap::new(),
            bundle_metrics: HashMap::new(),
        }
    }

    /// Parse a JSON log line and produce structured records.
    pub fn ingest_line(
        &mut self,
        line: &str,
    ) -> Result<Vec<TransparencyRecord>, TransparencyError> {
        let log_record: ResolverEventLog = norito::json::from_str(line)?;
        Ok(self.ingest_event(log_record))
    }

    fn ingest_event(&mut self, log_record: ResolverEventLog) -> Vec<TransparencyRecord> {
        let ResolverEventLog {
            timestamp,
            resolver_id,
            event,
        } = log_record;

        match event {
            ResolverEvent::BundleAdded { namehash, snapshot } => {
                let key = (resolver_id.clone(), namehash.clone());
                let previous = self.bundles.insert(key.clone(), snapshot.clone());
                let record = BundleRecord::new(
                    timestamp,
                    resolver_id.clone(),
                    namehash,
                    &snapshot,
                    BundleEventKind::Added,
                    previous.as_ref(),
                );
                self.apply_bundle_metrics(&key, &record);
                vec![TransparencyRecord::Bundle(Box::new(record))]
            }
            ResolverEvent::BundleUpdated {
                namehash,
                previous,
                current,
            } => {
                let key = (resolver_id.clone(), namehash.clone());
                let prior = self.bundles.insert(key.clone(), current.clone());
                let record = BundleRecord::new(
                    timestamp,
                    resolver_id.clone(),
                    namehash,
                    &current,
                    BundleEventKind::Updated,
                    prior.as_ref().or(Some(&previous)),
                );
                self.apply_bundle_metrics(&key, &record);
                vec![TransparencyRecord::Bundle(Box::new(record))]
            }
            ResolverEvent::BundleReorged {
                namehash,
                previous,
                current,
            } => {
                let key = (resolver_id.clone(), namehash.clone());
                let prior = self.bundles.insert(key.clone(), current.clone());
                let record = BundleRecord::new(
                    timestamp,
                    resolver_id.clone(),
                    namehash,
                    &current,
                    BundleEventKind::Reorged,
                    prior.as_ref().or(Some(&previous)),
                );
                self.apply_bundle_metrics(&key, &record);
                vec![TransparencyRecord::Bundle(Box::new(record))]
            }
            ResolverEvent::BundleRemoved { namehash, snapshot } => {
                let key = (resolver_id.clone(), namehash.clone());
                let prior = self.bundles.remove(&key);
                let record = BundleRecord::new(
                    timestamp,
                    resolver_id.clone(),
                    namehash,
                    &snapshot,
                    BundleEventKind::Removed,
                    prior.as_ref(),
                );
                self.apply_bundle_metrics(&key, &record);
                vec![TransparencyRecord::Bundle(Box::new(record))]
            }
            ResolverEvent::BundleExpired { namehash, snapshot } => {
                let key = (resolver_id.clone(), namehash.clone());
                let prior = self.bundles.remove(&key);
                let record = BundleRecord::new(
                    timestamp,
                    resolver_id.clone(),
                    namehash,
                    &snapshot,
                    BundleEventKind::Expired,
                    prior.as_ref(),
                );
                self.apply_bundle_metrics(&key, &record);
                vec![TransparencyRecord::Bundle(Box::new(record))]
            }
            ResolverEvent::ResolverAdded { resolver_id } => {
                vec![TransparencyRecord::Resolver(ResolverRecord {
                    timestamp,
                    resolver_id,
                    event: ResolverEventKind::Added,
                })]
            }
            ResolverEvent::ResolverUpdated { resolver_id } => {
                vec![TransparencyRecord::Resolver(ResolverRecord {
                    timestamp,
                    resolver_id,
                    event: ResolverEventKind::Updated,
                })]
            }
            ResolverEvent::ResolverRemoved { resolver_id } => {
                self.bundles
                    .retain(|(resolver, _), _| resolver != &resolver_id);
                self.bundle_metrics
                    .retain(|(resolver, _), _| resolver != &resolver_id);
                vec![TransparencyRecord::Resolver(ResolverRecord {
                    timestamp,
                    resolver_id,
                    event: ResolverEventKind::Removed,
                })]
            }
            ResolverEvent::ResolverInvalidated { resolver_id, .. } => {
                self.bundles
                    .retain(|(resolver, _), _| resolver != &resolver_id);
                self.bundle_metrics
                    .retain(|(resolver, _), _| resolver != &resolver_id);
                vec![TransparencyRecord::Resolver(ResolverRecord {
                    timestamp,
                    resolver_id,
                    event: ResolverEventKind::Invalidated,
                })]
            }
        }
    }

    fn apply_bundle_metrics(&mut self, key: &(String, String), record: &BundleRecord) {
        match record.event {
            BundleEventKind::Removed | BundleEventKind::Expired => {
                self.bundle_metrics.remove(key);
            }
            _ => {
                let entry = self.bundle_metrics.entry(key.clone()).or_insert_with(|| {
                    BundleMetricState::from_snapshot(&record.snapshot, record.timestamp)
                });
                entry.update(&record.snapshot, record.timestamp, record.cid_changed);
            }
        }
    }

    #[must_use]
    pub fn metrics(&self) -> Vec<MetricLine> {
        let mut keys: Vec<_> = self.bundle_metrics.keys().cloned().collect();
        keys.sort();
        let mut lines = Vec::with_capacity(keys.len() * 3);
        for (resolver_id, namehash) in keys {
            if let Some(state) = self
                .bundle_metrics
                .get(&(resolver_id.clone(), namehash.clone()))
            {
                let labels = vec![
                    ("resolver_id", resolver_id.clone()),
                    ("namehash", namehash.clone()),
                ];
                lines.push(MetricLine {
                    metric: "soradns_bundle_proof_age_seconds",
                    labels: labels.clone(),
                    value: state.proof_age_secs as f64,
                });
                lines.push(MetricLine {
                    metric: "soradns_bundle_proof_ttl_seconds",
                    labels: labels.clone(),
                    value: state.proof_ttl_secs as f64,
                });
                lines.push(MetricLine {
                    metric: "soradns_bundle_cid_drift_total",
                    labels,
                    value: state.cid_drift_total as f64,
                });
            }
        }
        lines
    }

    /// Snapshot the current bundle states tracked by the tailer.
    #[must_use]
    pub fn bundle_states(&self) -> Vec<BundleState> {
        let mut states = Vec::with_capacity(self.bundle_metrics.len());
        for ((resolver_id, namehash), metrics) in &self.bundle_metrics {
            states.push(BundleState {
                resolver_id: resolver_id.clone(),
                namehash: namehash.clone(),
                snapshot: metrics.snapshot.clone(),
                last_timestamp: metrics.last_timestamp,
                proof_age_secs: metrics.proof_age_secs,
                proof_ttl_secs: metrics.proof_ttl_secs,
                cid_drift_total: metrics.cid_drift_total,
            });
        }
        states.sort_by(|a, b| match a.resolver_id.cmp(&b.resolver_id) {
            Ordering::Equal => a.namehash.cmp(&b.namehash),
            other => other,
        });
        states
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::BundleSnapshot;

    fn sample_snapshot(version: u64, cid: &str) -> BundleSnapshot {
        BundleSnapshot {
            zone_version: version,
            manifest_hash_hex: "aa".into(),
            policy_hash_hex: "bb".into(),
            car_root_cid: cid.into(),
            freshness_issued_at: 10,
            freshness_expires_at: 50,
            freshness_signer: "council".into(),
            freshness_signature_hex: "ff".into(),
        }
    }

    #[test]
    fn tailer_generates_bundle_records() {
        let mut tailer = TransparencyTailer::new();
        let log = ResolverEventLog {
            timestamp: 20,
            resolver_id: "resolver-1".into(),
            event: ResolverEvent::BundleAdded {
                namehash: "deadbeef".into(),
                snapshot: sample_snapshot(7, "cid1"),
            },
        };
        let records = tailer.ingest_event(log);
        assert_eq!(records.len(), 1);
        let TransparencyRecord::Bundle(bundle) = &records[0] else {
            panic!("expected bundle record");
        };
        let bundle = bundle.as_ref();
        assert_eq!(bundle.resolver_id, "resolver-1");
        assert_eq!(bundle.zone_version, 7);
        assert_eq!(bundle.proof_age_secs, 10);
        assert_eq!(bundle.proof_ttl_secs, 30);
        assert!(!bundle.cid_changed);
        assert_eq!(bundle.event, BundleEventKind::Added);
        let json = bundle.to_json_value();
        assert_eq!(json["record_type"].as_str(), Some("bundle"));
        assert_eq!(json["resolver_id"].as_str(), Some("resolver-1"));

        let metrics = tailer.metrics();
        assert!(
            metrics.iter().any(|line| {
                line.metric == "soradns_bundle_proof_age_seconds"
                    && line
                        .labels
                        .contains(&("resolver_id", String::from("resolver-1")))
                    && line
                        .labels
                        .contains(&("namehash", String::from("deadbeef")))
                    && (line.value - 10.0).abs() < f64::EPSILON
            }),
            "expected proof age metric for resolver-1 deadbeef"
        );
    }

    #[test]
    fn bundle_states_snapshot_sorted() {
        let mut tailer = TransparencyTailer::new();
        let first = ResolverEventLog {
            timestamp: 30,
            resolver_id: "resolver-b".into(),
            event: ResolverEvent::BundleAdded {
                namehash: "beef".into(),
                snapshot: sample_snapshot(5, "cid-1"),
            },
        };
        tailer.ingest_event(first);
        let second = ResolverEventLog {
            timestamp: 40,
            resolver_id: "resolver-a".into(),
            event: ResolverEvent::BundleAdded {
                namehash: "cafe".into(),
                snapshot: sample_snapshot(8, "cid-2"),
            },
        };
        tailer.ingest_event(second);

        let mut states = tailer.bundle_states();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].resolver_id, "resolver-a");
        assert_eq!(states[0].namehash, "cafe");
        assert_eq!(states[0].snapshot.zone_version, 8);
        assert_eq!(states[0].proof_age_secs, 30);
        assert_eq!(states[1].resolver_id, "resolver-b");
        assert_eq!(states[1].namehash, "beef");
        assert_eq!(states[1].cid_drift_total, 0);

        // ingest drift update for resolver-b and confirm metrics carry over
        let third = ResolverEventLog {
            timestamp: 60,
            resolver_id: "resolver-b".into(),
            event: ResolverEvent::BundleUpdated {
                namehash: "beef".into(),
                previous: sample_snapshot(5, "cid-1"),
                current: sample_snapshot(6, "cid-2"),
            },
        };
        tailer.ingest_event(third);
        states = tailer.bundle_states();
        assert_eq!(states[1].snapshot.zone_version, 6);
        assert_eq!(states[1].cid_drift_total, 1);
        assert_eq!(states[1].last_timestamp, 60);
    }

    #[test]
    fn tailer_detects_cid_change_on_update() {
        let mut tailer = TransparencyTailer::new();
        // initial add
        let add_log = ResolverEventLog {
            timestamp: 15,
            resolver_id: "resolver-1".into(),
            event: ResolverEvent::BundleAdded {
                namehash: "deadbeef".into(),
                snapshot: sample_snapshot(1, "cid1"),
            },
        };
        tailer.ingest_event(add_log);
        // update with new CID
        let update_log = ResolverEventLog {
            timestamp: 25,
            resolver_id: "resolver-1".into(),
            event: ResolverEvent::BundleUpdated {
                namehash: "deadbeef".into(),
                previous: sample_snapshot(1, "cid1"),
                current: sample_snapshot(2, "cid2"),
            },
        };
        let records = tailer.ingest_event(update_log);
        assert_eq!(records.len(), 1);
        let TransparencyRecord::Bundle(bundle) = &records[0] else {
            panic!("expected bundle record");
        };
        let bundle = bundle.as_ref();
        assert!(bundle.cid_changed);
        assert_eq!(bundle.previous_zone_version, Some(1));
        assert_eq!(bundle.zone_version, 2);

        let metrics = tailer.metrics();
        assert!(
            metrics.iter().any(|line| {
                line.metric == "soradns_bundle_cid_drift_total"
                    && line
                        .labels
                        .contains(&("resolver_id", String::from("resolver-1")))
                    && line
                        .labels
                        .contains(&("namehash", String::from("deadbeef")))
                    && (line.value - 1.0).abs() < f64::EPSILON
            }),
            "expected CID drift counter to increment"
        );
    }

    #[test]
    fn tailer_removes_metrics_on_expiry() {
        let mut tailer = TransparencyTailer::new();
        let add_log = ResolverEventLog {
            timestamp: 15,
            resolver_id: "resolver-1".into(),
            event: ResolverEvent::BundleAdded {
                namehash: "deadbeef".into(),
                snapshot: sample_snapshot(1, "cid1"),
            },
        };
        tailer.ingest_event(add_log);

        let expire_log = ResolverEventLog {
            timestamp: 30,
            resolver_id: "resolver-1".into(),
            event: ResolverEvent::BundleExpired {
                namehash: "deadbeef".into(),
                snapshot: sample_snapshot(1, "cid1"),
            },
        };
        let records = tailer.ingest_event(expire_log);
        let TransparencyRecord::Bundle(record) = &records[0] else {
            panic!("expected bundle record");
        };
        assert_eq!(record.event, BundleEventKind::Expired);
        assert!(tailer.metrics().is_empty());
    }

    #[test]
    fn tailer_emits_resolver_invalidation() {
        let mut tailer = TransparencyTailer::new();
        tailer.ingest_event(ResolverEventLog {
            timestamp: 10,
            resolver_id: "resolver-1".into(),
            event: ResolverEvent::BundleAdded {
                namehash: "deadbeef".into(),
                snapshot: sample_snapshot(1, "cid1"),
            },
        });

        let log = ResolverEventLog {
            timestamp: 42,
            resolver_id: "resolver-1".into(),
            event: ResolverEvent::ResolverInvalidated {
                resolver_id: "resolver-1".into(),
                reason: "expired".into(),
            },
        };
        let records = tailer.ingest_event(log);
        let TransparencyRecord::Resolver(rec) = &records[0] else {
            panic!("expected resolver record");
        };
        assert_eq!(rec.event, ResolverEventKind::Invalidated);
        assert!(tailer.metrics().is_empty());
    }

    #[test]
    fn tailer_emits_resolver_records() {
        let mut tailer = TransparencyTailer::new();
        let log = ResolverEventLog {
            timestamp: 42,
            resolver_id: "resolver-1".into(),
            event: ResolverEvent::ResolverAdded {
                resolver_id: "resolver-1".into(),
            },
        };
        let records = tailer.ingest_event(log);
        assert_eq!(records.len(), 1);
        let TransparencyRecord::Resolver(rec) = &records[0] else {
            panic!("expected resolver record");
        };
        assert_eq!(rec.timestamp, 42);
        assert_eq!(rec.event, ResolverEventKind::Added);
        let tsv = rec.to_tsv_row();
        assert!(
            tsv.starts_with("resolver\t42"),
            "TSV output should start with resolver label"
        );
    }
}
