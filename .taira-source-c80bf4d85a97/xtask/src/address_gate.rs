use std::{collections::BTreeMap, fmt::Write as _, fs, io::Write as _};

use eyre::eyre;
use norito::json::{self, Value};

use crate::JsonTarget;

#[derive(Debug, Clone)]
pub struct LocalGateOptions {
    pub input: std::path::PathBuf,
    pub window_days: u64,
    pub json_out: Option<JsonTarget>,
    pub check_collisions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SeriesDelta {
    metric: String,
    labels: BTreeMap<String, String>,
    delta: u64,
}

#[derive(Debug, Clone)]
struct Series {
    name: String,
    labels: BTreeMap<String, String>,
    samples: Vec<Sample>,
}

#[derive(Debug, Clone)]
struct Sample {
    timestamp: f64,
    value: f64,
}

#[derive(Debug)]
struct GateReport {
    window_days: u64,
    coverage_days: f64,
    local8_series: usize,
    collision_series: usize,
    blockers: Vec<String>,
    offending_local8: Vec<SeriesDelta>,
    offending_collisions: Vec<SeriesDelta>,
}

impl GateReport {
    fn ready(&self) -> bool {
        self.blockers.is_empty()
    }

    fn blockers_message(&self) -> String {
        let mut message = String::new();
        for blocker in &self.blockers {
            let _ = writeln!(&mut message, "- {blocker}");
        }
        message
    }
}

pub fn run_local_gate(options: LocalGateOptions) -> eyre::Result<()> {
    let content = fs::read(&options.input).map_err(|err| {
        eyre!(
            "failed to read Prometheus response `{}`: {err}",
            options.input.display()
        )
    })?;
    let value: Value = json::from_slice(&content)
        .map_err(|err| eyre!("failed to parse Prometheus response JSON: {err}"))?;

    let report = build_report(&value, options.window_days, options.check_collisions)?;

    if let Some(target) = options.json_out.clone() {
        write_report_json(&report, target)?;
    }

    if report.ready() {
        println!(
            "Local-8/Local-12 gate ready: {}d coverage (required {}d), {} Local-8 series, {} collision series, no increments observed.",
            report.coverage_days, report.window_days, report.local8_series, report.collision_series
        );
        return Ok(());
    }

    // Emit blockers before returning an error.
    eprintln!("Local-8 gate not ready:");
    eprint!("{}", report.blockers_message());
    Err(eyre!("Local-8 gate blockers present (see above)"))
}

fn build_report(
    root: &Value,
    window_days: u64,
    check_collisions: bool,
) -> eyre::Result<GateReport> {
    let series = parse_matrix(root)?;
    let mut local8_series = Vec::new();
    let mut collision_series = Vec::new();
    for entry in series {
        match entry.name.as_str() {
            "torii_address_local8_total" => local8_series.push(entry),
            "torii_address_collision_total" if check_collisions => collision_series.push(entry),
            _ => {}
        }
    }

    let mut blockers = Vec::new();
    if check_collisions && collision_series.is_empty() {
        blockers.push(
            "collision checking enabled but no `torii_address_collision_total` series were present"
                .to_string(),
        );
    }

    let coverage_seconds = coverage_window_seconds(&local8_series, &collision_series);
    let required_seconds = (window_days * 24 * 60 * 60) as f64;
    if coverage_seconds < required_seconds {
        blockers.push(format!(
            "insufficient coverage window: saw {:.2} days, require {window_days} days",
            coverage_seconds / 86_400.0
        ));
    }

    let offenders = collect_offenders(&local8_series, "Local-8 counter");
    let offending_local8 = offenders.offenders;
    if !offending_local8.is_empty() {
        blockers.push(format!(
            "detected {} Local-8 increments (see offending series for contexts)",
            offending_local8.len()
        ));
    }
    if offenders.resets_detected {
        blockers.push("one or more Local-8 series decreased (counter reset); collect a longer window before enabling the gate".to_string());
    }

    let collision_offenders = collect_offenders(&collision_series, "Local-12 collision counter");
    let offending_collisions = collision_offenders.offenders;
    if !offending_collisions.is_empty() {
        blockers.push(format!(
            "detected {} Local-12 collision increments (see offending series for contexts)",
            offending_collisions.len()
        ));
    }
    if collision_offenders.resets_detected {
        blockers.push(
            "one or more Local-12 collision series decreased (counter reset); collect a longer window before enabling the gate"
                .to_string(),
        );
    }

    Ok(GateReport {
        window_days,
        coverage_days: coverage_seconds / 86_400.0,
        local8_series: local8_series.len(),
        collision_series: collision_series.len(),
        blockers,
        offending_local8,
        offending_collisions,
    })
}

fn write_report_json(report: &GateReport, target: JsonTarget) -> eyre::Result<()> {
    let mut root = json::Map::new();
    root.insert("ready".into(), norito::json!(report.ready()));
    root.insert("window_days".into(), norito::json!(report.window_days));
    root.insert("coverage_days".into(), norito::json!(report.coverage_days));

    let mut series = json::Map::new();
    series.insert("local8".into(), norito::json!(report.local8_series as u64));
    series.insert(
        "collision".into(),
        norito::json!(report.collision_series as u64),
    );
    root.insert("series_seen".into(), json::Value::Object(series));

    root.insert("blockers".into(), norito::json!(report.blockers));
    root.insert(
        "offending_local8".into(),
        norito::json!(
            report
                .offending_local8
                .iter()
                .map(delta_to_json)
                .collect::<Vec<_>>()
        ),
    );
    root.insert(
        "offending_collisions".into(),
        norito::json!(
            report
                .offending_collisions
                .iter()
                .map(delta_to_json)
                .collect::<Vec<_>>()
        ),
    );

    let bytes = json::to_vec_pretty(&json::Value::Object(root))
        .map_err(|err| eyre!("failed to serialize gate report: {err}"))?;

    match target {
        JsonTarget::Stdout => {
            std::io::stdout()
                .write_all(&bytes)
                .map_err(|err| eyre!("failed to write JSON report to stdout: {err}"))?;
            println!();
        }
        JsonTarget::File(path) => {
            fs::write(&path, &bytes).map_err(|err| {
                eyre!("failed to write JSON report to `{}`: {err}", path.display())
            })?;
        }
    }
    Ok(())
}

fn delta_to_json(delta: &SeriesDelta) -> Value {
    let mut labels = json::Map::new();
    for (key, value) in &delta.labels {
        labels.insert(key.clone(), norito::json!(value));
    }
    let mut obj = json::Map::new();
    obj.insert("metric".into(), norito::json!(delta.metric.as_str()));
    obj.insert("labels".into(), Value::Object(labels));
    obj.insert("delta".into(), norito::json!(delta.delta));
    Value::Object(obj)
}

fn parse_matrix(root: &Value) -> eyre::Result<Vec<Series>> {
    let data = root
        .get("data")
        .ok_or_else(|| eyre!("Prometheus response missing `data` field"))?;
    let result = data
        .get("result")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("Prometheus response missing `data.result` array"))?;

    let mut series = Vec::new();
    for entry in result {
        let metric = entry
            .get("metric")
            .and_then(Value::as_object)
            .ok_or_else(|| eyre!("matrix entry missing `metric` object"))?;
        let values = entry
            .get("values")
            .or_else(|| entry.get("value"))
            .ok_or_else(|| eyre!("matrix entry missing `values` array"))?;

        let name = metric
            .get("__name__")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("matrix entry missing __name__ label"))?
            .to_string();
        let mut labels = BTreeMap::new();
        for (key, value) in metric {
            if key == "__name__" {
                continue;
            }
            if let Some(label) = value.as_str() {
                labels.insert(key.clone(), label.to_string());
            }
        }

        let samples = parse_samples(values)?;
        series.push(Series {
            name,
            labels,
            samples,
        });
    }

    Ok(series)
}

fn parse_samples(values: &Value) -> eyre::Result<Vec<Sample>> {
    let raw_entries = values
        .as_array()
        .ok_or_else(|| eyre!("matrix entry `values` must be an array"))?;

    let first_is_array = matches!(raw_entries.first(), Some(Value::Array(_)));

    let entries: Vec<Value> = if raw_entries.len() == 2 && !first_is_array {
        vec![Value::Array(raw_entries.to_vec())]
    } else {
        raw_entries.to_vec()
    };

    let mut samples = Vec::new();
    for entry in entries {
        let pair = entry
            .as_array()
            .ok_or_else(|| eyre!("sample entry was not an array"))?;
        if pair.len() != 2 {
            return Err(eyre!(
                "sample entry had {} elements (expected 2)",
                pair.len()
            ));
        }
        let timestamp = parse_f64(&pair[0]).map_err(|err| eyre!("invalid timestamp: {err}"))?;
        let value = parse_f64(&pair[1]).map_err(|err| eyre!("invalid value: {err}"))?;
        samples.push(Sample { timestamp, value });
    }

    if samples.is_empty() {
        return Err(eyre!("series had no samples"));
    }

    samples.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());
    Ok(samples)
}

fn parse_f64(value: &Value) -> Result<f64, String> {
    if let Some(number) = value.as_f64() {
        return Ok(number);
    }
    if let Some(text) = value.as_str() {
        return text
            .parse::<f64>()
            .map_err(|err| format!("failed to parse `{text}` as float: {err}"));
    }
    Err(format!("unexpected value type: {value:?}"))
}

fn coverage_window_seconds(local8: &[Series], collisions: &[Series]) -> f64 {
    let timestamps = local8
        .iter()
        .chain(collisions.iter())
        .flat_map(|series| series.samples.iter().map(|sample| sample.timestamp));
    let mut min_ts = f64::INFINITY;
    let mut max_ts = f64::NEG_INFINITY;
    for ts in timestamps {
        if ts < min_ts {
            min_ts = ts;
        }
        if ts > max_ts {
            max_ts = ts;
        }
    }
    if !min_ts.is_finite() || !max_ts.is_finite() {
        return 0.0;
    }
    (max_ts - min_ts).max(0.0)
}

struct OffenderReport {
    offenders: Vec<SeriesDelta>,
    resets_detected: bool,
}

fn collect_offenders(series: &[Series], metric_label: &str) -> OffenderReport {
    let mut offenders = Vec::new();
    let mut resets_detected = false;
    for entry in series {
        let first = entry
            .samples
            .first()
            .map(|sample| sample.value)
            .unwrap_or(0.0);
        let last = entry
            .samples
            .last()
            .map(|sample| sample.value)
            .unwrap_or(0.0);
        let delta = last - first;
        if delta > 0.0 {
            offenders.push(SeriesDelta {
                metric: metric_label.to_string(),
                labels: entry.labels.clone(),
                delta: delta as u64,
            });
        } else if delta < 0.0 {
            resets_detected = true;
        }
    }
    OffenderReport {
        offenders,
        resets_detected,
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn write_json_to_temp(value: &Value) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("input.json");
        let bytes = json::to_vec(value).expect("serialize test JSON");
        fs::write(&path, bytes).expect("write test file");
        (dir, path)
    }

    fn response_with_results(results: Value) -> Value {
        norito::json!({
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": results
            }
        })
    }

    #[test]
    fn gate_ready_when_counters_flat() {
        let response = response_with_results(norito::json!([
            {
                "metric": {"__name__": "torii_address_local8_total", "context": "/v1/accounts"},
                "values": [[0, "5"], [2_678_400f64, "5"]] // 31 days apart
            },
            {
                "metric": {"__name__": "torii_address_collision_total", "context": "/v1/accounts", "kind": "local12_digest"},
                "values": [[0, "0"], [2_678_400f64, "0"]]
            }
        ]));

        let (_dir, path) = write_json_to_temp(&response);
        let report = build_report(&response, 30, true).expect("report builds");
        assert!(
            report.ready(),
            "report should be ready when counters stay flat"
        );

        let options = LocalGateOptions {
            input: path,
            window_days: 30,
            json_out: None,
            check_collisions: true,
        };
        run_local_gate(options).expect("gate should pass");
    }

    #[test]
    fn gate_fails_on_local8_growth() {
        let response = response_with_results(norito::json!([
            {
                "metric": {"__name__": "torii_address_local8_total", "context": "/v1/assets"},
                "values": [[0, "1"], [2_592_000f64, "3"]]
            },
            {
                "metric": {"__name__": "torii_address_collision_total", "context": "/v1/assets", "kind": "local12_digest"},
                "values": [[0, "0"], [2_592_000f64, "0"]]
            }
        ]));

        let (_dir, path) = write_json_to_temp(&response);
        let result = run_local_gate(LocalGateOptions {
            input: path,
            window_days: 30,
            json_out: None,
            check_collisions: true,
        });
        assert!(
            result.is_err(),
            "non-zero Local-8 increments must fail the gate"
        );
    }

    #[test]
    fn gate_fails_on_short_window() {
        let response = response_with_results(norito::json!([
            {
                "metric": {"__name__": "torii_address_local8_total", "context": "/v1/accounts"},
                "values": [[0, "1"], [86_400f64, "1"]] // 1 day
            },
            {
                "metric": {"__name__": "torii_address_collision_total", "context": "/v1/accounts", "kind": "local12_digest"},
                "values": [[0, "0"], [86_400f64, "0"]]
            }
        ]));

        let (_dir, path) = write_json_to_temp(&response);
        let result = run_local_gate(LocalGateOptions {
            input: path,
            window_days: 30,
            json_out: None,
            check_collisions: true,
        });
        assert!(result.is_err(), "insufficient coverage must block the gate");
    }

    #[test]
    fn gate_warns_when_collision_series_missing() {
        let response = response_with_results(norito::json!([
            {
                "metric": {"__name__": "torii_address_local8_total", "context": "/v1/accounts"},
                "values": [[0, "1"], [2_678_400f64, "1"]]
            }
        ]));

        let (_dir, path) = write_json_to_temp(&response);
        let result = run_local_gate(LocalGateOptions {
            input: path,
            window_days: 30,
            json_out: None,
            check_collisions: true,
        });
        assert!(
            result.is_err(),
            "missing collision series should prevent success when checking collisions"
        );
    }
}
