//! Validate SM performance benchmark output against reference medians.

use std::{
    collections::BTreeMap,
    convert::TryFrom,
    env,
    error::Error,
    fmt::Write as _,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use norito::json::{self, Map, Value};

struct BenchSpec {
    key: &'static str,
    path: &'static [&'static str],
}

const BENCH_SPECS: &[BenchSpec] = &[
    BenchSpec {
        key: "sm2_vs_ed25519_sign/sm2_sign",
        path: &["sm2_vs_ed25519_sign", "sm2_sign"],
    },
    BenchSpec {
        key: "sm2_vs_ed25519_sign/ed25519_sign",
        path: &["sm2_vs_ed25519_sign", "ed25519_sign"],
    },
    BenchSpec {
        key: "sm2_vs_ed25519_verify/verify/sm2",
        path: &["sm2_vs_ed25519_verify", "verify", "sm2"],
    },
    BenchSpec {
        key: "sm2_vs_ed25519_verify/verify/ed25519",
        path: &["sm2_vs_ed25519_verify", "verify", "ed25519"],
    },
    BenchSpec {
        key: "sm3_vs_sha256_hash/sm3_hash",
        path: &["sm3_vs_sha256_hash", "sm3_hash"],
    },
    BenchSpec {
        key: "sm3_vs_sha256_hash/sha256_hash",
        path: &["sm3_vs_sha256_hash", "sha256_hash"],
    },
    BenchSpec {
        key: "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt",
        path: &["sm4_vs_chacha20poly1305_encrypt", "sm4_gcm_encrypt"],
    },
    BenchSpec {
        key: "sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt",
        path: &[
            "sm4_vs_chacha20poly1305_encrypt",
            "chacha20poly1305_encrypt",
        ],
    },
    BenchSpec {
        key: "sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt",
        path: &["sm4_vs_chacha20poly1305_decrypt", "sm4_gcm_decrypt"],
    },
    BenchSpec {
        key: "sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt",
        path: &[
            "sm4_vs_chacha20poly1305_decrypt",
            "chacha20poly1305_decrypt",
        ],
    },
];

struct BaselineData {
    metadata: Option<BaselineMetadata>,
    benchmarks: BTreeMap<String, f64>,
    tolerances: Option<BTreeMap<String, f64>>,
    compare_tolerances: Option<BTreeMap<String, f64>>,
}

#[derive(Default)]
struct BaselineMetadata {
    target_arch: Option<String>,
    target_os: Option<String>,
    cpu: Option<String>,
    notes: Option<String>,
}

struct ResultRow {
    key: String,
    measured: f64,
    reference: f64,
    compare: Option<f64>,
}

struct CliArgs {
    criterion_dir: PathBuf,
    baseline_path: PathBuf,
    tolerance: f64,
    require_summary: bool,
    write_baseline: Option<PathBuf>,
    summary_target: Option<PathBuf>,
    compare_baseline: Option<PathBuf>,
    compare_tolerance: f64,
    compare_label: Option<String>,
    capture_json: Option<PathBuf>,
    capture_mode: Option<String>,
    capture_label: Option<String>,
}

struct CaptureMetadata<'a> {
    baseline_path: &'a Path,
    compare_baseline: Option<&'a Path>,
    mode: Option<&'a str>,
    label: Option<&'a str>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let Some(cli) = parse_args()? else {
        return Ok(());
    };

    let baseline = load_baseline(&cli.baseline_path)?;
    warn_on_mismatched_metadata(baseline.metadata.as_ref());
    let compare_baseline = if let Some(path) = cli.compare_baseline.as_deref() {
        Some(load_baseline(path)?)
    } else {
        None
    };

    let results = collect_results(&cli.criterion_dir, &baseline, compare_baseline.as_ref())?;
    emit_summary(&results);

    if let Some(path) = cli.capture_json.as_deref() {
        let mode = cli.capture_mode.as_deref();
        let label = cli.capture_label.as_deref().or(mode);
        write_capture_json(
            &results,
            &CaptureMetadata {
                baseline_path: &cli.baseline_path,
                compare_baseline: cli.compare_baseline.as_deref(),
                mode,
                label,
            },
            path,
        )?;
        println!("Capture summary written to {}", path.display());
    }

    let compare_label = compare_baseline.as_ref().map(|_| {
        cli.compare_label.clone().unwrap_or_else(|| {
            cli.compare_baseline
                .as_ref()
                .expect("cli compare baseline present when loaded")
                .display()
                .to_string()
        })
    });

    if let Some(path) = cli.summary_target.as_deref() {
        if let Err(err) = append_summary_markdown(
            &results,
            path,
            compare_label.as_deref(),
            compare_baseline.as_ref().map(|_| cli.compare_tolerance),
        ) {
            eprintln!("warning: failed to write GitHub summary ({err})");
        }
    } else if cli.require_summary {
        return Err("GITHUB_STEP_SUMMARY not set; cannot write summary".into());
    }

    if let Some(path) = cli.write_baseline.as_deref() {
        write_baseline_file(&results, path)?;
        println!("Updated baseline written to {}", path.display());
    }

    report_failures(&results, cli.tolerance, baseline.tolerances.as_ref())?;

    if let (Some(compare_data), Some(label)) = (compare_baseline.as_ref(), compare_label.as_deref())
    {
        println!(
            "Comparison baseline: {label} (tolerance {}%).",
            cli.compare_tolerance * 100.0
        );
        report_comparison_failures(
            &results,
            label,
            cli.compare_tolerance,
            compare_data
                .compare_tolerances
                .as_ref()
                .or(compare_data.tolerances.as_ref()),
        )?;
    }

    Ok(())
}

fn parse_args() -> Result<Option<CliArgs>, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut cli = CliArgs {
        criterion_dir: PathBuf::from("target/criterion"),
        baseline_path: PathBuf::from("crates/iroha_crypto/benches/sm_perf_baseline.json"),
        tolerance: 0.25_f64,
        require_summary: false,
        write_baseline: None,
        summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),
        compare_baseline: None,
        compare_tolerance: 0.10_f64,
        compare_label: None,
        capture_json: None,
        capture_mode: None,
        capture_label: None,
    };
    let mut show_help = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--criterion-dir" => {
                let value = args.next().ok_or("missing value for --criterion-dir")?;
                cli.criterion_dir = PathBuf::from(value);
            }
            "--baseline" => {
                let value = args.next().ok_or("missing value for --baseline")?;
                cli.baseline_path = PathBuf::from(value);
            }
            "--tolerance" => {
                let value = args.next().ok_or("missing value for --tolerance")?;
                cli.tolerance = value.parse::<f64>()?;
            }
            "--compare-baseline" => {
                let value = args.next().ok_or("missing value for --compare-baseline")?;
                cli.compare_baseline = Some(PathBuf::from(value));
            }
            "--compare-tolerance" => {
                let value = args.next().ok_or("missing value for --compare-tolerance")?;
                cli.compare_tolerance = value.parse::<f64>()?;
            }
            "--capture-json" => {
                let value = args.next().ok_or("missing value for --capture-json")?;
                cli.capture_json = Some(PathBuf::from(value));
            }
            "--capture-mode" => {
                let value = args.next().ok_or("missing value for --capture-mode")?;
                cli.capture_mode = Some(value);
            }
            "--capture-label" => {
                let value = args.next().ok_or("missing value for --capture-label")?;
                cli.capture_label = Some(value);
            }
            "--compare-label" => {
                let value = args.next().ok_or("missing value for --compare-label")?;
                cli.compare_label = Some(value);
            }
            "--summary-only" => {
                cli.require_summary = true;
            }
            "--write-baseline" => {
                let value = args.next().ok_or("missing value for --write-baseline")?;
                cli.write_baseline = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                show_help = true;
            }
            other => {
                return Err(format!("unknown argument: {other}").into());
            }
        }
    }

    if show_help {
        print_help();
        return Ok(None);
    }

    Ok(Some(cli))
}

fn warn_on_mismatched_metadata(meta: Option<&BaselineMetadata>) {
    let Some(meta) = meta else { return };

    if let Some(target_arch) = meta.target_arch.as_deref() {
        let arch = env::consts::ARCH;
        if target_arch != arch {
            eprintln!(
                "warning: baseline recorded for arch '{target_arch}' but host arch is '{arch}'"
            );
        }
    }
    if let Some(target_os) = meta.target_os.as_deref() {
        let os = env::consts::OS;
        if target_os != os {
            eprintln!("warning: baseline recorded for os '{target_os}' but host os is '{os}'");
        }
    }
}

fn collect_results(
    criterion_dir: &Path,
    baseline: &BaselineData,
    compare: Option<&BaselineData>,
) -> Result<Vec<ResultRow>, Box<dyn Error>> {
    let mut results = Vec::with_capacity(BENCH_SPECS.len());
    for spec in BENCH_SPECS {
        let reference = baseline
            .benchmarks
            .get(spec.key)
            .ok_or_else(|| format!("baseline missing entry for {}", spec.key))?;
        let compare_value = if let Some(compare) = compare {
            Some(
                *compare
                    .benchmarks
                    .get(spec.key)
                    .ok_or_else(|| format!("comparison baseline missing entry for {}", spec.key))?,
            )
        } else {
            None
        };
        let measured = load_measurement(criterion_dir, spec)?;
        results.push(ResultRow {
            key: spec.key.to_owned(),
            measured,
            reference: *reference,
            compare: compare_value,
        });
    }
    Ok(results)
}

fn report_failures(
    results: &[ResultRow],
    default_tolerance: f64,
    overrides: Option<&BTreeMap<String, f64>>,
) -> Result<(), Box<dyn Error>> {
    let mut failures = Vec::new();
    let mut used_override = false;
    for row in results {
        let tolerance = overrides
            .and_then(|map| map.get(&row.key))
            .copied()
            .unwrap_or(default_tolerance);
        if (tolerance - default_tolerance).abs() > f64::EPSILON {
            used_override = true;
        }
        let allowed = row.reference * (1.0 + tolerance);
        if row.measured > allowed {
            failures.push(format!(
                "{key}: observed {measured:.2} µs exceeds allowed {allowed:.2} µs \
                 (baseline {reference:.2} µs, tolerance {percent:.2}%)",
                key = row.key,
                measured = row.measured,
                allowed = allowed,
                reference = row.reference,
                percent = tolerance * 100.0
            ));
        }
    }

    if failures.is_empty() {
        if used_override {
            println!(
                "All SM benchmark medians within tolerance (default {default:.2}%, overrides applied where specified).",
                default = default_tolerance * 100.0
            );
        } else {
            println!(
                "All SM benchmark medians within tolerance ({:.2}%).",
                default_tolerance * 100.0
            );
        }
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn print_help() {
    println!(
        "Usage: sm_perf_check [--criterion-dir PATH] [--baseline PATH] [--tolerance FRACTION] \
         [--summary-only] [--write-baseline PATH] [--compare-baseline PATH] \
         [--compare-tolerance FRACTION] [--compare-label LABEL] [--capture-json PATH] \
         [--capture-mode MODE] [--capture-label LABEL]"
    );
    println!("Defaults:");
    println!("  --criterion-dir target/criterion");
    println!("  --baseline crates/iroha_crypto/benches/sm_perf_baseline.json");
    println!("  --tolerance 0.25   # allow 25% slowdown relative to baseline");
    println!("  --compare-tolerance 0.10   # allow 10% slowdown relative to comparison baseline");
    println!("Use --summary-only to require writing to $GITHUB_STEP_SUMMARY (failing if unset).");
    println!("Use --write-baseline to export current measurements as a new baseline file.");
    println!(
        "Use --capture-json PATH to emit structured measurements for later aggregation (optionally add --capture-mode/--capture-label)."
    );
}

fn load_baseline(path: &Path) -> Result<BaselineData, Box<dyn Error>> {
    let payload = fs::read_to_string(path)?;
    let value: Value = norito::json::from_str(&payload)?;

    let metadata = value
        .get("metadata")
        .and_then(Value::as_object)
        .map(|object| {
            let mut meta = BaselineMetadata::default();
            if let Some(target_arch) = object.get("target_arch").and_then(Value::as_str) {
                meta.target_arch = Some(target_arch.to_owned());
            }
            if let Some(target_os) = object.get("target_os").and_then(Value::as_str) {
                meta.target_os = Some(target_os.to_owned());
            }
            if let Some(cpu) = object.get("cpu").and_then(Value::as_str) {
                meta.cpu = Some(cpu.to_owned());
            }
            if let Some(notes) = object.get("notes").and_then(Value::as_str) {
                meta.notes = Some(notes.to_owned());
            }
            meta
        });

    let benchmarks = value
        .get("benchmarks")
        .and_then(Value::as_object)
        .ok_or("baseline file missing 'benchmarks' object")?;

    let mut map = BTreeMap::new();
    for (name, entry) in benchmarks {
        let median = entry
            .as_f64()
            .ok_or_else(|| format!("baseline value for {name} is not a number"))?;
        map.insert(name.clone(), median);
    }

    let tolerances = if let Some(value) = value.get("tolerances") {
        let object = value
            .as_object()
            .ok_or("baseline 'tolerances' must be an object")?;
        let mut overrides = BTreeMap::new();
        for (name, entry) in object {
            let tolerance = entry
                .as_f64()
                .ok_or_else(|| format!("baseline tolerance for {name} is not a number"))?;
            overrides.insert(name.clone(), tolerance);
        }
        Some(overrides)
    } else {
        None
    };

    let compare_tolerances = if let Some(value) = value.get("compare_tolerances") {
        let object = value
            .as_object()
            .ok_or("baseline 'compare_tolerances' must be an object")?;
        let mut overrides = BTreeMap::new();
        for (name, entry) in object {
            let tolerance = entry
                .as_f64()
                .ok_or_else(|| format!("baseline compare_tolerance for {name} is not a number"))?;
            overrides.insert(name.clone(), tolerance);
        }
        Some(overrides)
    } else {
        None
    };

    Ok(BaselineData {
        metadata,
        benchmarks: map,
        tolerances,
        compare_tolerances,
    })
}

fn load_measurement(dir: &Path, spec: &BenchSpec) -> Result<f64, Box<dyn Error>> {
    let mut path = PathBuf::from(dir);
    for component in spec.path {
        path.push(component);
    }
    path.push("new");
    path.push("estimates.json");

    let payload = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read measurement for {}: {err}", spec.key))?;
    let value: Value = norito::json::from_str(&payload)?;
    let median = value
        .get("median")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("point_estimate"))
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            format!(
                "estimates.json for {} missing median.point_estimate",
                spec.key
            )
        })?;

    // Criterion stores nanoseconds in `estimates.json`; convert to microseconds.
    Ok(median / 1_000.0)
}

fn emit_summary(results: &[ResultRow]) {
    println!("SM benchmark medians (microseconds):");
    let has_compare = results.iter().any(|row| row.compare.is_some());
    if has_compare {
        println!("| Benchmark | Measured | Baseline | Δ vs Baseline | Compare | Δ vs Compare |");
        println!("|-----------|----------|----------|----------------|---------|--------------|");
    } else {
        println!("| Benchmark | Measured | Baseline | Δ vs Baseline |");
        println!("|-----------|----------|----------|----------------|");
    }
    for row in results {
        let baseline_delta = ((row.measured - row.reference) / row.reference) * 100.0;
        if let Some(compare) = row.compare {
            let compare_delta = ((row.measured - compare) / compare) * 100.0;
            println!(
                "| {} | {:>8.2} | {:>8.2} | {:+12.2}% | {:>7.2} | {:+12.2}% |",
                row.key, row.measured, row.reference, baseline_delta, compare, compare_delta
            );
        } else {
            println!(
                "| {} | {:>8.2} | {:>8.2} | {:+12.2}% |",
                row.key, row.measured, row.reference, baseline_delta
            );
        }
    }
}

fn append_summary_markdown(
    results: &[ResultRow],
    summary_path: &Path,
    compare_label: Option<&str>,
    compare_tolerance: Option<f64>,
) -> Result<(), Box<dyn Error>> {
    let mut buffer = String::new();
    buffer.push_str("### SM Benchmark Medians\n\n");
    let has_compare = results.iter().any(|row| row.compare.is_some());
    if has_compare {
        buffer.push_str(
            "| Benchmark | Measured (µs) | Baseline (µs) | Δ vs Baseline | Compare (µs) | Δ vs Compare |\n",
        );
        buffer.push_str(
            "|-----------|---------------|---------------|----------------|--------------|--------------|\n",
        );
    } else {
        buffer.push_str("| Benchmark | Measured (µs) | Baseline (µs) | Δ vs Baseline |\n");
        buffer.push_str("|-----------|---------------|---------------|----------------|\n");
    }
    for row in results {
        let delta = ((row.measured - row.reference) / row.reference) * 100.0;
        if let Some(compare) = row.compare {
            let compare_delta = ((row.measured - compare) / compare) * 100.0;
            let _ = writeln!(
                &mut buffer,
                "| {} | {:>13.2} | {:>13.2} | {:+12.2}% | {:>12.2} | {:+12.2}% |",
                row.key, row.measured, row.reference, delta, compare, compare_delta
            );
        } else {
            let _ = writeln!(
                &mut buffer,
                "| {} | {:>13.2} | {:>13.2} | {:+12.2}% |",
                row.key, row.measured, row.reference, delta
            );
        }
    }
    if let (Some(label), Some(tolerance)) = (compare_label, compare_tolerance) {
        let _ = writeln!(
            &mut buffer,
            "\nComparison baseline: {label} (tolerance {percent:.0}%).",
            percent = tolerance * 100.0
        );
    }

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(summary_path)?;
    file.write_all(buffer.as_bytes())?;
    Ok(())
}

fn report_comparison_failures(
    results: &[ResultRow],
    compare_label: &str,
    default_tolerance: f64,
    overrides: Option<&BTreeMap<String, f64>>,
) -> Result<(), Box<dyn Error>> {
    let mut failures = Vec::new();
    let mut used_override = false;
    for row in results {
        let Some(compare) = row.compare else { continue };
        let tolerance = overrides
            .and_then(|map| map.get(&row.key))
            .copied()
            .unwrap_or(default_tolerance);
        if (tolerance - default_tolerance).abs() > f64::EPSILON {
            used_override = true;
        }
        let allowed = compare * (1.0 + tolerance);
        if row.measured > allowed {
            failures.push(format!(
                "{key}: observed {measured:.2} µs exceeds allowed {allowed:.2} µs \
                 (comparison baseline {compare:.2} µs from {label}, tolerance {percent:.0}%)",
                key = row.key,
                measured = row.measured,
                allowed = allowed,
                compare = compare,
                label = compare_label,
                percent = tolerance * 100.0,
            ));
        }
    }

    if failures.is_empty() {
        if used_override {
            println!(
                "Acceleration guard against {label} passed (default {percent:.2}%, overrides applied where specified).",
                label = compare_label,
                percent = default_tolerance * 100.0
            );
        } else {
            println!(
                "Acceleration guard against {label} passed (tolerance {percent:.2}%).",
                label = compare_label,
                percent = default_tolerance * 100.0
            );
        }
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn write_baseline_file(results: &[ResultRow], path: &Path) -> Result<(), Box<dyn Error>> {
    let mut metadata = Map::new();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());
    metadata.insert("target_arch".to_owned(), Value::from(arch));
    metadata.insert("target_os".to_owned(), Value::from(os));

    if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {
        metadata.insert("cpu".to_owned(), Value::from(cpu));
    }

    let mut benchmarks = Map::new();
    for row in results {
        benchmarks.insert(row.key.clone(), Value::from(row.measured));
    }

    let mut root = Map::new();
    root.insert("metadata".to_owned(), Value::Object(metadata));
    root.insert("benchmarks".to_owned(), Value::Object(benchmarks));

    let json = Value::Object(root);
    let content = json::to_json_pretty(&json)?;
    fs::write(path, content)?;
    Ok(())
}

fn write_capture_json(
    results: &[ResultRow],
    metadata: &CaptureMetadata<'_>,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let mut meta = Map::new();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());
    meta.insert("target_arch".to_owned(), Value::from(arch));
    meta.insert("target_os".to_owned(), Value::from(os));

    if let Some(mode) = metadata.mode {
        meta.insert("mode".to_owned(), Value::from(mode));
    }
    if let Some(label) = metadata.label {
        meta.insert("label".to_owned(), Value::from(label));
    }
    if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {
        meta.insert("cpu_label".to_owned(), Value::from(cpu));
    }

    let baseline_display = metadata.baseline_path.display().to_string();
    meta.insert("baseline_path".to_owned(), Value::from(baseline_display));
    if let Some(compare_path) = metadata.compare_baseline {
        meta.insert(
            "compare_baseline_path".to_owned(),
            Value::from(compare_path.display().to_string()),
        );
    }

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis())
        .unwrap_or(0);
    let timestamp_clamped = i64::try_from(timestamp_ms).unwrap_or(i64::MAX);
    meta.insert(
        "generated_unix_ms".to_owned(),
        Value::from(timestamp_clamped),
    );

    let mut measured_map = Map::new();
    let mut reference_map = Map::new();
    let mut compare_map = Map::new();
    let mut has_compare = false;
    for row in results {
        measured_map.insert(row.key.clone(), Value::from(row.measured));
        reference_map.insert(row.key.clone(), Value::from(row.reference));
        if let Some(compare) = row.compare {
            compare_map.insert(row.key.clone(), Value::from(compare));
            has_compare = true;
        }
    }

    let mut root = Map::new();
    root.insert("metadata".to_owned(), Value::Object(meta));
    root.insert("benchmarks".to_owned(), Value::Object(measured_map));
    root.insert("reference".to_owned(), Value::Object(reference_map));
    if has_compare {
        root.insert("comparison".to_owned(), Value::Object(compare_map));
    }

    let json = Value::Object(root);
    fs::write(path, json::to_json_pretty(&json)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn load_baseline_parses_metadata_and_values() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().join("baseline.json");
        let payload = r#"{
            "metadata": {
                "target_arch": "aarch64",
                "target_os": "macos",
                "cpu": "Test CPU"
            },
            "benchmarks": {
                "sm2_vs_ed25519_sign/sm2_sign": 100.0,
                "sm3_vs_sha256_hash/sm3_hash": 10.5
            },
            "tolerances": {
                "sm2_vs_ed25519_sign/sm2_sign": 0.15,
                "sm3_vs_sha256_hash/sm3_hash": 0.05
            },
            "compare_tolerances": {
                "sm3_vs_sha256_hash/sm3_hash": 0.50
            }
        }"#;
        fs::write(&path, payload).expect("write baseline");

        let baseline = load_baseline(&path).expect("load baseline");
        assert_eq!(
            baseline
                .metadata
                .as_ref()
                .and_then(|m| m.target_arch.as_ref()),
            Some(&"aarch64".to_owned())
        );
        assert_eq!(
            baseline
                .metadata
                .as_ref()
                .and_then(|m| m.target_os.as_ref()),
            Some(&"macos".to_owned())
        );
        assert_eq!(
            baseline.benchmarks.get("sm2_vs_ed25519_sign/sm2_sign"),
            Some(&100.0)
        );
        assert_eq!(
            baseline.benchmarks.get("sm3_vs_sha256_hash/sm3_hash"),
            Some(&10.5)
        );
        assert_eq!(
            baseline
                .tolerances
                .as_ref()
                .and_then(|map| map.get("sm2_vs_ed25519_sign/sm2_sign")),
            Some(&0.15)
        );
        assert_eq!(
            baseline
                .tolerances
                .as_ref()
                .and_then(|map| map.get("sm3_vs_sha256_hash/sm3_hash")),
            Some(&0.05)
        );
        assert_eq!(
            baseline
                .compare_tolerances
                .as_ref()
                .and_then(|map| map.get("sm3_vs_sha256_hash/sm3_hash")),
            Some(&0.50)
        );
    }

    #[test]
    fn write_capture_json_outputs_expected_payload() {
        let temp = TempDir::new().expect("tempdir");
        let capture_path = temp.path().join("capture.json");
        let baseline_path = temp.path().join("baseline.json");
        fs::write(&baseline_path, "{}").expect("baseline stub");

        let rows = vec![
            ResultRow {
                key: "sm2_vs_ed25519_sign/sm2_sign".to_owned(),
                measured: 100.0,
                reference: 120.0,
                compare: Some(90.0),
            },
            ResultRow {
                key: "sm3_vs_sha256_hash/sm3_hash".to_owned(),
                measured: 11.0,
                reference: 10.0,
                compare: None,
            },
        ];

        write_capture_json(
            &rows,
            &CaptureMetadata {
                baseline_path: &baseline_path,
                compare_baseline: None,
                mode: Some("scalar"),
                label: Some("scalar"),
            },
            &capture_path,
        )
        .expect("capture json");

        let payload = fs::read_to_string(&capture_path).expect("capture payload");
        let value: Value = json::from_str(&payload).expect("parsed capture payload");
        let metadata = value
            .get("metadata")
            .and_then(Value::as_object)
            .expect("metadata block");
        assert_eq!(metadata.get("mode").and_then(Value::as_str), Some("scalar"));
        assert_eq!(
            metadata
                .get("baseline_path")
                .and_then(Value::as_str)
                .map(|path| path.ends_with("baseline.json")),
            Some(true)
        );
        let benchmarks = value
            .get("benchmarks")
            .and_then(Value::as_object)
            .expect("benchmarks block");
        assert_eq!(
            benchmarks["sm3_vs_sha256_hash/sm3_hash"].as_f64(),
            Some(11.0)
        );
        assert!(value.get("comparison").is_some());
    }

    #[test]
    fn load_measurement_converts_nanoseconds_to_microseconds() {
        let temp = TempDir::new().expect("tempdir");
        let mut path = temp.path().to_path_buf();
        for component in &["sm2_vs_ed25519_sign", "sm2_sign", "new"] {
            path.push(component);
            fs::create_dir(&path).expect("mkdir");
        }
        let estimates = r#"{
            "median": {
                "point_estimate": 12345.0
            }
        }"#;
        fs::write(path.join("estimates.json"), estimates).expect("write estimates");

        let spec = &BenchSpec {
            key: "sm2_vs_ed25519_sign/sm2_sign",
            path: &["sm2_vs_ed25519_sign", "sm2_sign"],
        };
        let measurement = load_measurement(temp.path(), spec).expect("load measurement");
        assert!((measurement - 12.345).abs() < f64::EPSILON);
    }

    #[test]
    fn per_benchmark_tolerance_override_passes() {
        let rows = vec![ResultRow {
            key: "bench".to_owned(),
            measured: 110.0,
            reference: 100.0,
            compare: None,
        }];
        let mut overrides = BTreeMap::new();
        overrides.insert("bench".to_owned(), 0.20);
        report_failures(&rows, 0.05, Some(&overrides))
            .expect("override should allow slower benchmark within custom tolerance");
    }

    #[test]
    fn per_benchmark_tolerance_override_fails_when_exceeded() {
        let rows = vec![ResultRow {
            key: "bench".to_owned(),
            measured: 130.0,
            reference: 100.0,
            compare: None,
        }];
        let mut overrides = BTreeMap::new();
        overrides.insert("bench".to_owned(), 0.20);
        let err = report_failures(&rows, 0.05, Some(&overrides))
            .expect_err("exceeding override tolerance should still fail");
        assert!(
            err.to_string()
                .contains("observed 130.00 µs exceeds allowed 120.00 µs")
        );
    }

    #[test]
    fn comparison_guard_passes_within_tolerance() {
        let rows = vec![ResultRow {
            key: "bench".to_owned(),
            measured: 10.0,
            reference: 9.0,
            compare: Some(10.5),
        }];
        report_comparison_failures(&rows, "scalar baseline", 0.10, None)
            .expect("comparison within tolerance");
    }

    #[test]
    fn comparison_guard_fails_on_excessive_slowdown() {
        let rows = vec![ResultRow {
            key: "bench".to_owned(),
            measured: 12.0,
            reference: 9.0,
            compare: Some(10.0),
        }];
        let err = report_comparison_failures(&rows, "scalar baseline", 0.10, None)
            .expect_err("comparison should fail");
        let message = err.to_string();
        assert!(message.contains("bench"));
        assert!(message.contains("scalar baseline"));
    }
}
