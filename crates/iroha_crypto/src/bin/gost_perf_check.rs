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
    let value: Value = norito::json::from_str(&payload)?;
    let algorithms = value
        .get("algorithms")
        .and_then(Value::as_object)
        .ok_or("baseline file missing 'algorithms' object")?;

    let mut map = BTreeMap::new();
    for (name, entry) in algorithms {
        let value = entry
            .as_f64()
            .ok_or_else(|| format!("baseline value for {name} is not a number"))?;
        map.insert(name.clone(), value);
    }
    Ok(map)
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
    Ok(median / 1_000.0)
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

#[allow(dead_code)]
fn _algorithm_from_name(name: &str) -> Option<Algorithm> {
    match name {
        "ed25519" => Some(Algorithm::Ed25519),
        "secp256k1" => Some(Algorithm::Secp256k1),
        "gost256_paramset_a" => Some(Algorithm::Gost3410_2012_256ParamSetA),
        "gost256_paramset_b" => Some(Algorithm::Gost3410_2012_256ParamSetB),
        "gost256_paramset_c" => Some(Algorithm::Gost3410_2012_256ParamSetC),
        "gost512_paramset_a" => Some(Algorithm::Gost3410_2012_512ParamSetA),
        "gost512_paramset_b" => Some(Algorithm::Gost3410_2012_512ParamSetB),
        _ => None,
    }
}
