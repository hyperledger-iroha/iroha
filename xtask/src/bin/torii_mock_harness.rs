#![allow(clippy::print_stdout)]

use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use eyre::{Context, Result, bail, eyre};
use hex::encode as hex_encode;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const DEFAULT_SCENARIO: &str = "submit";
const DEFAULT_TEST: &str = "org.hyperledger.iroha.android.client.HttpClientTransportHarnessTests";
const DEFAULT_CONFIG_RELATIVE: &str = "tools/torii_mock_harness/config/default.toml";
const DEFAULT_FIXTURE_RELATIVE: &str =
    "java/iroha_android/src/test/resources/transaction_payloads.json";
const DEFAULT_RUNNER_RELATIVE: &str = "java/iroha_android/run_tests.sh";

fn main() -> Result<()> {
    let opts = CliOptions::parse()?;
    let repo_root = locate_repo_root(opts.repo_root.as_deref())?;
    let config_path = opts
        .config_path
        .clone()
        .unwrap_or_else(|| repo_root.join(DEFAULT_CONFIG_RELATIVE));
    let scenarios = ScenarioSet::load(&config_path)?;
    let scenario = scenarios.resolve(&opts.scenario)?;

    let tests = opts.tests.clone().unwrap_or_else(|| scenario.tests.clone());
    if tests.is_empty() {
        bail!("scenario '{}' does not specify any tests", scenario.name);
    }

    let harness_runner = resolve_repo_path(&repo_root, &opts.runner, &scenario.runner);
    if !harness_runner.is_file() {
        bail!("expected harness runner at {}", harness_runner.display());
    }

    let fixture_path = resolve_repo_path(&repo_root, &opts.fixture_path, &scenario.fixture);
    if !fixture_path.is_file() {
        bail!("fixture payloads not found at {}", fixture_path.display());
    }

    let metrics_path = opts
        .metrics_path
        .clone()
        .unwrap_or_else(|| default_metrics_path(&repo_root, &opts.sdk));

    let joined_tests = tests.join(",");
    let start = Instant::now();
    run_harness(
        &harness_runner,
        &repo_root,
        &joined_tests,
        &opts.sdk,
        &scenario.name,
        &metrics_path,
        &scenario.env,
    )?;
    let duration_ms = start.elapsed().as_millis() as u64;

    let fixture_hash = compute_sha256_hex(&fixture_path)?;
    let retry_total = read_retry_total();
    write_metrics_file(
        &metrics_path,
        &opts.sdk,
        &scenario.name,
        duration_ms,
        retry_total,
        &fixture_hash,
    )?;

    println!(
        "[torii-mock-harness] Scenario '{}' for SDK '{}' completed in {}ms",
        scenario.name, opts.sdk, duration_ms
    );
    println!(
        "[torii-mock-harness] Metrics written to {}",
        metrics_path.display()
    );
    Ok(())
}

#[derive(Debug, Default, Clone)]
struct CliOptions {
    sdk: String,
    scenario: String,
    tests: Option<Vec<String>>,
    metrics_path: Option<PathBuf>,
    config_path: Option<PathBuf>,
    repo_root: Option<PathBuf>,
    runner: Option<PathBuf>,
    fixture_path: Option<PathBuf>,
}

impl CliOptions {
    fn parse() -> Result<Self> {
        let mut args = env::args().skip(1);
        let mut opts = CliOptions {
            sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),
            scenario: DEFAULT_SCENARIO.to_string(),
            metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")
                .ok()
                .map(PathBuf::from),
            repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")
                .ok()
                .map(PathBuf::from),
            runner: env::var("TORII_MOCK_HARNESS_RUNNER")
                .ok()
                .map(PathBuf::from),
            ..CliOptions::default()
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--sdk" => {
                    opts.sdk = args.next().ok_or_else(|| eyre!("--sdk requires a value"))?;
                }
                "--scenario" => {
                    opts.scenario = args
                        .next()
                        .ok_or_else(|| eyre!("--scenario requires a value"))?;
                }
                "--tests" => {
                    let raw = args
                        .next()
                        .ok_or_else(|| eyre!("--tests requires a value"))?;
                    opts.tests = Some(parse_tests(&raw));
                }
                "--metrics-path" => {
                    let value = args
                        .next()
                        .ok_or_else(|| eyre!("--metrics-path requires a value"))?;
                    opts.metrics_path = Some(PathBuf::from(value));
                }
                "--config" => {
                    let value = args
                        .next()
                        .ok_or_else(|| eyre!("--config requires a value"))?;
                    opts.config_path = Some(PathBuf::from(value));
                }
                "--repo-root" => {
                    let value = args
                        .next()
                        .ok_or_else(|| eyre!("--repo-root requires a value"))?;
                    opts.repo_root = Some(PathBuf::from(value));
                }
                "--runner" => {
                    let value = args
                        .next()
                        .ok_or_else(|| eyre!("--runner requires a value"))?;
                    opts.runner = Some(PathBuf::from(value));
                }
                "--fixture" => {
                    let value = args
                        .next()
                        .ok_or_else(|| eyre!("--fixture requires a value"))?;
                    opts.fixture_path = Some(PathBuf::from(value));
                }
                "-h" | "--help" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => {
                    return Err(eyre!("unknown argument '{other}'"));
                }
            }
        }

        Ok(opts)
    }
}

fn print_usage() {
    println!(
        "\
torii-mock-harness — run the cross-SDK mock harness smoke suite.

Usage:
  torii-mock-harness [--sdk <name>] [--scenario <name>]
                     [--tests <comma-separated-class-list>]
                     [--metrics-path <path>] [--config <path>]
                     [--repo-root <path>] [--fixture <path>]
                     [--runner <path>]

Environment:
  TORII_MOCK_HARNESS_SDK             Override the SDK label (default: android)
  TORII_MOCK_HARNESS_METRICS_PATH    Location for the emitted Prometheus metrics file
  TORII_MOCK_HARNESS_REPO_ROOT       Explicit repository root (falls back to auto-detect)
  TORII_MOCK_HARNESS_RUNNER          Override the harness runner script/binary
  TORII_MOCK_HARNESS_RETRY_TOTAL     Overrides retry counter published in metrics (default: 0)\
"
    );
}

fn parse_tests(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect()
}

fn locate_repo_root(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(root) = explicit {
        return fs::canonicalize(root)
            .with_context(|| format!("failed to canonicalize {}", root.display()));
    }
    let cwd = env::current_dir().context("failed to determine current directory")?;
    if let Some(path) = walk_up_to_repo(&cwd) {
        return Ok(path);
    }
    Err(eyre!(
        "failed to locate repository root; pass --repo-root to override"
    ))
}

fn walk_up_to_repo(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let cargo = dir.join("Cargo.toml");
        let tool_dir = dir.join("tools").join("torii_mock_harness");
        if cargo.is_file() && tool_dir.is_dir() {
            return fs::canonicalize(dir).ok();
        }
        current = dir.parent();
    }
    None
}

fn resolve_repo_path(
    repo_root: &Path,
    override_path: &Option<PathBuf>,
    default_path: &Path,
) -> PathBuf {
    if let Some(path) = override_path {
        return join_repo(repo_root, path);
    }
    join_repo(repo_root, default_path)
}

fn join_repo(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn default_metrics_path(repo_root: &Path, sdk: &str) -> PathBuf {
    repo_root.join(format!("mock-harness-metrics-{sdk}.prom"))
}

fn run_harness(
    script: &Path,
    repo_root: &Path,
    tests: &str,
    sdk: &str,
    scenario_name: &str,
    metrics_path: &Path,
    extra_env: &HashMap<String, String>,
) -> Result<()> {
    let mut cmd = Command::new(script);
    cmd.arg("--tests").arg(tests);
    cmd.current_dir(repo_root);
    cmd.env("TORII_MOCK_HARNESS_SDK", sdk);
    cmd.env("TORII_MOCK_HARNESS_SCENARIO", scenario_name);
    cmd.env("TORII_MOCK_HARNESS_METRICS_PATH", metrics_path);
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    let status = cmd
        .status()
        .with_context(|| format!("failed to execute {}", script.display()))?;
    if !status.success() {
        if let Some(code) = status.code() {
            bail!("harness runner exited with status {code}");
        }
        bail!("harness runner terminated by signal");
    }
    Ok(())
}

fn compute_sha256_hex(path: &Path) -> Result<String> {
    let data = fs::read(path)
        .with_context(|| format!("failed to read fixture payload {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex_encode(hasher.finalize()))
}

fn read_retry_total() -> u64 {
    env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(0)
}

fn write_metrics_file(
    path: &Path,
    sdk: &str,
    scenario: &str,
    duration_ms: u64,
    retry_total: u64,
    fixture_hash: &str,
) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let contents = format!(
        "# HELP torii_mock_harness_duration_ms Execution time for mock harness scenarios.\n\
# TYPE torii_mock_harness_duration_ms gauge\n\
torii_mock_harness_duration_ms{{sdk=\"{sdk}\",scenario=\"{scenario}\"}} {duration_ms}\n\
# HELP torii_mock_harness_retry_total Number of retries triggered during mock harness smoke tests.\n\
# TYPE torii_mock_harness_retry_total counter\n\
torii_mock_harness_retry_total{{sdk=\"{sdk}\",scenario=\"{scenario}\"}} {retry_total}\n\
# HELP torii_mock_harness_fixture_version Fixture bundle hash used for the run.\n\
# TYPE torii_mock_harness_fixture_version gauge\n\
torii_mock_harness_fixture_version{{sdk=\"{sdk}\",scenario=\"{scenario}\",fixture_version=\"{fixture_hash}\"}} 1\n"
    );
    fs::write(path, contents)
        .with_context(|| format!("failed to write metrics file {}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ScenarioSet {
    entries: HashMap<String, ScenarioEntry>,
}

impl ScenarioSet {
    fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file: ScenarioFile =
            toml::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))?;
        let mut entries = HashMap::new();
        for (index, record) in file.scenario.into_iter().enumerate() {
            let key = record.name.trim().to_string();
            if key.is_empty() {
                bail!("scenario entry #{index} is missing a name");
            }
            if entries.contains_key(&key) {
                bail!("duplicate scenario '{key}' in config");
            }
            entries.insert(key.clone(), record.into_entry());
        }
        Ok(Self { entries })
    }

    fn resolve(&self, name: &str) -> Result<&ScenarioEntry> {
        self.entries
            .get(name)
            .ok_or_else(|| eyre!("scenario '{}' not found in config", name))
    }
}

#[derive(Debug, Deserialize)]
struct ScenarioFile {
    #[serde(default)]
    scenario: Vec<ScenarioRecord>,
}

#[derive(Debug, Deserialize)]
struct ScenarioRecord {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tests: Vec<String>,
    #[serde(default)]
    runner: Option<String>,
    #[serde(default)]
    fixture: Option<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

impl ScenarioRecord {
    fn into_entry(self) -> ScenarioEntry {
        let tests = sanitize_tests(self.tests);
        let runner =
            sanitize_path(self.runner).unwrap_or_else(|| PathBuf::from(DEFAULT_RUNNER_RELATIVE));
        let fixture =
            sanitize_path(self.fixture).unwrap_or_else(|| PathBuf::from(DEFAULT_FIXTURE_RELATIVE));
        ScenarioEntry {
            name: self.name,
            description: self.description,
            tests: if tests.is_empty() {
                vec![DEFAULT_TEST.to_string()]
            } else {
                tests
            },
            runner,
            fixture,
            env: self.env,
        }
    }
}

fn sanitize_tests(raw: Vec<String>) -> Vec<String> {
    raw.into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect()
}

fn sanitize_path(raw: Option<String>) -> Option<PathBuf> {
    raw.and_then(|entry| {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    })
}

#[derive(Debug, Clone)]
struct ScenarioEntry {
    name: String,
    #[allow(dead_code)]
    description: Option<String>,
    tests: Vec<String>,
    runner: PathBuf,
    fixture: PathBuf,
    env: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_config_parses() {
        let toml = r#"
[[scenario]]
name = "demo"
tests = ["FooTest", " BarTest "]

[[scenario]]
name = "fallback"
"#;
        let file: ScenarioFile = toml::from_str(toml).expect("parse");
        let entries: Vec<_> = file
            .scenario
            .into_iter()
            .map(ScenarioRecord::into_entry)
            .collect();
        assert_eq!(entries[0].tests, vec!["FooTest", "BarTest"]);
        assert_eq!(entries[1].tests, vec![DEFAULT_TEST]);
        assert_eq!(entries[0].runner, PathBuf::from(DEFAULT_RUNNER_RELATIVE));
        assert_eq!(entries[0].fixture, PathBuf::from(DEFAULT_FIXTURE_RELATIVE));
    }

    #[test]
    fn scenario_config_resolves_runner_and_fixture() {
        let toml = r#"
[[scenario]]
name = "custom"
runner = "bin/run.sh"
fixture = "/tmp/fixture.json"
"#;
        let file: ScenarioFile = toml::from_str(toml).expect("parse");
        let entries: Vec<_> = file
            .scenario
            .into_iter()
            .map(ScenarioRecord::into_entry)
            .collect();
        assert_eq!(entries[0].runner, PathBuf::from("bin/run.sh"));
        assert_eq!(entries[0].fixture, PathBuf::from("/tmp/fixture.json"));
    }

    #[test]
    fn parse_tests_splits_and_trims() {
        let tests = parse_tests("Foo, Bar ,Baz");
        assert_eq!(tests, vec!["Foo", "Bar", "Baz"]);
        assert!(parse_tests(" , ").is_empty());
    }

    #[test]
    fn retry_total_parsing_handles_invalid() {
        unsafe {
            env::set_var("TORII_MOCK_HARNESS_RETRY_TOTAL", "5");
        }
        assert_eq!(read_retry_total(), 5);
        unsafe {
            env::set_var("TORII_MOCK_HARNESS_RETRY_TOTAL", "not-a-number");
        }
        assert_eq!(read_retry_total(), 0);
        unsafe {
            env::remove_var("TORII_MOCK_HARNESS_RETRY_TOTAL");
        }
    }
}
