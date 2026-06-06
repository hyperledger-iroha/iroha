//! Validate GOST benchmark output against reference medians with a configurable tolerance.

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fmt::Write as _,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use iroha_crypto::Algorithm;
use norito::json::{self, Map, Value};

const EXPECTED_ALGORITHMS: &[(&str, Algorithm)] = &[
    ("ed25519", Algorithm::Ed25519),
    ("secp256k1", Algorithm::Secp256k1),
    ("gost256_paramset_a", Algorithm::Gost3410_2012_256ParamSetA),
    ("gost256_paramset_b", Algorithm::Gost3410_2012_256ParamSetB),
    ("gost256_paramset_c", Algorithm::Gost3410_2012_256ParamSetC),
    ("gost512_paramset_a", Algorithm::Gost3410_2012_512ParamSetA),
    ("gost512_paramset_b", Algorithm::Gost3410_2012_512ParamSetB),
];

fn main() -> Result<(), Box<dyn Error>> {
    let mut criterion_dir = PathBuf::from("target/criterion");
    let mut baseline_path = PathBuf::from("crates/iroha_crypto/benches/gost_perf_baseline.json");
    let mut tolerance = 0.20_f64;
    let mut require_summary = false;
    let mut write_baseline = None;
    let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--criterion-dir" => {
                let value = args.next().ok_or("missing value for --criterion-dir")?;
                criterion_dir = PathBuf::from(value);
            }
            "--baseline" => {
                let value = args.next().ok_or("missing value for --baseline")?;
                baseline_path = PathBuf::from(value);
            }
            "--tolerance" => {
                let value = args.next().ok_or("missing value for --tolerance")?;
                tolerance = value.parse::<f64>()?;
            }
            "--summary-only" => {
                require_summary = true;
            }
            "--write-baseline" => {
                let value = args.next().ok_or("missing value for --write-baseline")?;
                write_baseline = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => {
                return Err(format!("unknown argument: {other}").into());
            }
        }
    }
    validate_tolerance(tolerance)?;

    let baselines = load_baselines(&baseline_path)?;
    let mut results = Vec::new();
    for (name, reference) in baselines {
        let measured = load_measurement(&criterion_dir, &name)?;
        results.push(ResultRow {
            name,
            measured,
            reference,
        });
    }

    emit_summary(&results);

    if let Some(path) = summary_target.as_deref() {
        if let Err(err) = append_summary_markdown(&results, path) {
            eprintln!("warning: failed to write GitHub summary ({err})");
        }
    } else if require_summary {
        return Err("GITHUB_STEP_SUMMARY not set; cannot write summary".into());
    }

    if let Some(path) = write_baseline {
        write_baseline_file(&results, &path)?;
        println!("Updated baseline written to {}", path.display());
    }

    let failures: Vec<String> = results
        .iter()
        .filter_map(|row| {
            let allowed = row.reference * (1.0 + tolerance);
            if row.measured > allowed {
                Some(format!(
                    "{name}: observed {measured:.2} µs exceeds allowed {allowed:.2} µs \
                     (baseline {reference:.2} µs, tolerance {tol:.0}%)",
                    name = row.name,
                    measured = row.measured,
                    allowed = allowed,
                    reference = row.reference,
                    tol = tolerance * 100.0
                ))
            } else {
                None
            }
        })
        .collect();

    if failures.is_empty() {
        println!(
            "All GOST benchmark medians within tolerance ({}%).",
            tolerance * 100.0
        );
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn print_help() {
    println!(
        "Usage: gost_perf_check [--criterion-dir PATH] [--baseline PATH] [--tolerance FRACTION] [--summary-only] [--write-baseline PATH]"
    );
    println!("Defaults:");
    println!("  --criterion-dir target/criterion");
    println!("  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json");
    println!("  --tolerance 0.20   # allow 20% slowdown relative to baseline");
    println!("Use --summary-only to require writing to $GITHUB_STEP_SUMMARY (failing if unset).");
    println!("Use --write-baseline to export current measurements as a new baseline file.");
}

fn load_baselines(path: &Path) -> Result<BTreeMap<String, f64>, Box<dyn Error>> {
    let payload = fs::read_to_string(path)?;
    parse_baselines(&payload)
}

fn parse_baselines(payload: &str) -> Result<BTreeMap<String, f64>, Box<dyn Error>> {
    let value: Value = norito::json::from_str(payload)?;
    let algorithms = value
        .get("algorithms")
        .and_then(Value::as_object)
        .ok_or("baseline file missing 'algorithms' object")?;

    let mut map = BTreeMap::new();
    for (name, entry) in algorithms {
        if algorithm_from_name(name).is_none() {
            return Err(format!(
                "baseline algorithm `{name}` is not in the GOST benchmark target set"
            )
            .into());
        }
        let value = entry
            .as_f64()
            .ok_or_else(|| format!("baseline value for {name} is not a number"))?;
        validate_positive_finite(
            value,
            &format!("baseline value for {name} must be finite and positive"),
        )?;
        map.insert(name.clone(), value);
    }
    validate_baseline_coverage(&map)?;
    Ok(map)
}

fn validate_baseline_coverage(baselines: &BTreeMap<String, f64>) -> Result<(), Box<dyn Error>> {
    for (name, _) in EXPECTED_ALGORITHMS {
        if !baselines.contains_key(*name) {
            return Err(format!("baseline missing required algorithm `{name}`").into());
        }
    }
    Ok(())
}

fn load_measurement(dir: &Path, name: &str) -> Result<f64, Box<dyn Error>> {
    // Criterion writes `.../<group>/<id>/<function>/new/estimates.json`.
    let estimates_path = dir
        .join("sign_verify")
        .join("sign_verify")
        .join(name)
        .join("new")
        .join("estimates.json");

    let payload = fs::read_to_string(&estimates_path).map_err(|err| {
        format!(
            "failed to read measurement for {name} at {}: {err}",
            estimates_path.display()
        )
    })?;
    let value: Value = norito::json::from_str(&payload)?;
    let median = value
        .get("median")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("point_estimate"))
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("estimates.json for {name} missing median.point_estimate"))?;

    // Criterion stores nanoseconds in `estimates.json`; convert to microseconds.
    let measured = median / 1_000.0;
    validate_positive_finite(
        measured,
        &format!("measurement median for {name} must be finite and positive"),
    )?;
    Ok(measured)
}

fn validate_tolerance(tolerance: f64) -> Result<(), Box<dyn Error>> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err("--tolerance must be a finite non-negative decimal".into());
    }
    Ok(())
}

fn validate_positive_finite(value: f64, message: &str) -> Result<(), Box<dyn Error>> {
    if !value.is_finite() || value <= 0.0 {
        return Err(message.to_owned().into());
    }
    Ok(())
}

struct ResultRow {
    name: String,
    measured: f64,
    reference: f64,
}

fn emit_summary(results: &[ResultRow]) {
    println!("GOST benchmark medians (microseconds):");
    println!("| Algorithm | Measured | Baseline | Delta |");
    println!("|-----------|----------|----------|-------|");
    for row in results {
        let delta = ((row.measured - row.reference) / row.reference) * 100.0;
        println!(
            "| {} | {:>8.2} | {:>8.2} | {:+6.2}% |",
            row.name, row.measured, row.reference, delta
        );
    }
}

fn append_summary_markdown(
    results: &[ResultRow],
    summary_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut buffer = String::new();
    buffer.push_str("### GOST Benchmark Medians\n\n");
    buffer.push_str("| Algorithm | Measured (µs) | Baseline (µs) | Delta |\n");
    buffer.push_str("|-----------|---------------|---------------|-------|\n");
    for row in results {
        let delta = ((row.measured - row.reference) / row.reference) * 100.0;
        let _ = writeln!(
            &mut buffer,
            "| {} | {:>11.2} | {:>11.2} | {:+6.2}% |",
            row.name, row.measured, row.reference, delta
        );
    }

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(summary_path)?;
    file.write_all(buffer.as_bytes())?;
    Ok(())
}

fn write_baseline_file(results: &[ResultRow], path: &Path) -> Result<(), Box<dyn Error>> {
    let mut algorithms = BTreeMap::new();
    for row in results {
        algorithms.insert(row.name.clone(), row.measured);
    }

    let mut map = Map::new();
    for (name, value) in algorithms {
        map.insert(name, Value::from(value));
    }

    let mut root = Map::new();
    root.insert("algorithms".to_owned(), Value::Object(map));
    let json = Value::Object(root);
    let content = json::to_json_pretty(&json)?;
    fs::write(path, content)?;
    Ok(())
}

fn algorithm_from_name(name: &str) -> Option<Algorithm> {
    EXPECTED_ALGORITHMS
        .iter()
        .find_map(|(expected, algorithm)| (*expected == name).then_some(*algorithm))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_payload(entries: &[(&str, f64)]) -> String {
        let mut payload = String::from(r#"{"algorithms":{"#);
        for (index, (name, value)) in entries.iter().enumerate() {
            if index > 0 {
                payload.push(',');
            }
            let _ = write!(&mut payload, r#""{name}":{value}"#);
        }
        payload.push_str("}}");
        payload
    }

    fn complete_baseline_entries() -> Vec<(&'static str, f64)> {
        EXPECTED_ALGORITHMS
            .iter()
            .map(|(name, _)| (*name, 1.0))
            .collect()
    }

    #[test]
    fn checked_in_gost_baseline_has_exact_target_coverage() {
        let raw = include_str!("../../benches/gost_perf_baseline.json");
        let baselines = parse_baselines(raw).expect("checked-in GOST baseline must parse");

        assert_eq!(baselines.len(), EXPECTED_ALGORITHMS.len());
        for (name, _) in EXPECTED_ALGORITHMS {
            assert!(
                baselines.contains_key(*name),
                "baseline missing expected algorithm {name}"
            );
        }
    }

    #[test]
    fn parse_baselines_rejects_missing_required_algorithm() {
        let mut entries = complete_baseline_entries();
        entries.pop();
        let err = parse_baselines(&baseline_payload(&entries))
            .expect_err("missing benchmark target must fail closed");

        assert!(
            err.to_string().contains("missing required algorithm"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_baselines_rejects_unknown_algorithm() {
        let mut entries = complete_baseline_entries();
        entries.push(("retired_gost_curve", 1.0));
        let err = parse_baselines(&baseline_payload(&entries))
            .expect_err("unknown benchmark target must fail closed");

        assert!(
            err.to_string()
                .contains("not in the GOST benchmark target set"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_baselines_rejects_non_positive_values() {
        let mut entries = complete_baseline_entries();
        entries[0].1 = 0.0;
        let err = parse_baselines(&baseline_payload(&entries))
            .expect_err("non-positive baseline value must fail closed");

        assert!(
            err.to_string().contains("finite and positive"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_tolerance_rejects_invalid_values() {
        for tolerance in [-0.01, f64::NAN, f64::INFINITY] {
            let err = validate_tolerance(tolerance)
                .expect_err("invalid tolerance must fail before comparison");
            assert!(
                err.to_string().contains("finite non-negative"),
                "unexpected error: {err}"
            );
        }
        validate_tolerance(0.0).expect("zero tolerance is valid");
        validate_tolerance(0.20).expect("positive finite tolerance is valid");
    }

    #[test]
    fn load_measurement_rejects_non_positive_medians() {
        let temp = tempfile::tempdir().expect("tempdir");
        let estimates_dir = temp
            .path()
            .join("sign_verify")
            .join("sign_verify")
            .join("ed25519")
            .join("new");
        fs::create_dir_all(&estimates_dir).expect("create estimates dir");
        fs::write(
            estimates_dir.join("estimates.json"),
            r#"{"median":{"point_estimate":0}}"#,
        )
        .expect("write estimates");

        let err = load_measurement(temp.path(), "ed25519")
            .expect_err("zero measurement median must fail closed");
        assert!(
            err.to_string().contains("finite and positive"),
            "unexpected error: {err}"
        );
    }
}
