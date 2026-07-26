use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::i3_bench_suite::{I3BenchOptions, I3BenchReport, run_i3_bench_suite};

/// Options for the Iroha 3 SLO harness.
#[derive(Clone, Debug)]
pub struct SloHarnessOptions {
    pub iterations: u32,
    pub sample_size: u32,
    pub out_dir: PathBuf,
    pub budgets: PathBuf,
    pub threshold: PathBuf,
    pub allow_overwrite: bool,
    pub flamegraph_hint: bool,
}

#[derive(Clone, Debug, JsonDeserialize)]
struct SloBudgetFile {
    targets: Vec<SloTarget>,
}

#[derive(Clone, Debug, JsonDeserialize)]
struct SloTarget {
    id: String,
    label: Option<String>,
    scenario: String,
    objective_ms: f64,
    description: Option<String>,
    burn_rate_fast: Option<f64>,
    burn_rate_slow: Option<f64>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct SloOutcome {
    id: String,
    label: String,
    scenario: String,
    objective_ms: f64,
    observed_ms: Option<f64>,
    status: String,
    budget_ratio: Option<f64>,
    burn_rate_fast: Option<f64>,
    burn_rate_slow: Option<f64>,
    note: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct SloReport {
    timestamp: String,
    git_hash: Option<String>,
    iterations: u32,
    sample_size: u32,
    outcomes: Vec<SloOutcome>,
}

/// Run the SLO harness by invoking the I3 bench suite, then mapping results to SLO objectives.
pub fn run_i3_slo_harness(options: SloHarnessOptions) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(&options.out_dir)?;
    let bench_json = options.out_dir.join("bench_report.json");
    let bench_csv = options.out_dir.join("bench_report.csv");
    let bench_md = options.out_dir.join("bench_report.md");

    let bench_opts = I3BenchOptions {
        iterations: options.iterations,
        sample_size: options.sample_size,
        json_out: bench_json.clone(),
        csv_out: Some(bench_csv),
        markdown_out: Some(bench_md),
        allow_overwrite: options.allow_overwrite,
        threshold: Some(options.threshold.clone()),
        flamegraph_hint: options.flamegraph_hint,
    };

    let bench_report = run_i3_bench_suite(bench_opts)?;
    let budgets = load_budgets(&options.budgets)?;
    let slo_report = evaluate_slos(&bench_report, budgets);

    let slo_json = options.out_dir.join("slo_report.json");
    let slo_md = options.out_dir.join("slo_report.md");
    write_json(&slo_report, &slo_json, options.allow_overwrite)?;
    write_markdown(&slo_report, &slo_md, options.allow_overwrite)?;

    Ok(())
}

fn load_budgets(path: &Path) -> Result<SloBudgetFile, Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    let budgets: SloBudgetFile = json::from_str(&contents)?;
    Ok(budgets)
}

fn evaluate_slos(report: &I3BenchReport, budgets: SloBudgetFile) -> SloReport {
    let mut by_name: HashMap<&str, &crate::i3_bench_suite::ScenarioResult> = HashMap::new();
    for scenario in &report.scenarios {
        by_name.insert(scenario.name.as_str(), scenario);
    }

    let mut outcomes = Vec::with_capacity(budgets.targets.len());
    for target in budgets.targets {
        let observed = by_name.get(target.scenario.as_str());
        let observed_ms = observed.map(|s| s.nanos_per_iter as f64 / 1_000_000.0);
        let (status, ratio) = if let Some(value) = observed_ms {
            if value <= target.objective_ms {
                ("pass".to_owned(), Some(value / target.objective_ms))
            } else {
                ("fail".to_owned(), Some(value / target.objective_ms))
            }
        } else {
            ("missing".to_owned(), None)
        };
        let label = target.label.clone().unwrap_or_else(|| target.id.clone());
        outcomes.push(SloOutcome {
            id: target.id,
            label,
            scenario: target.scenario,
            objective_ms: target.objective_ms,
            observed_ms,
            status,
            budget_ratio: ratio,
            burn_rate_fast: target.burn_rate_fast,
            burn_rate_slow: target.burn_rate_slow,
            note: target.description,
        });
    }

    SloReport {
        timestamp: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".into()),
        git_hash: report.git_hash.clone(),
        iterations: report.config.iterations,
        sample_size: report.config.sample_size,
        outcomes,
    }
}

fn write_json(
    report: &SloReport,
    path: &Path,
    allow_overwrite: bool,
) -> Result<(), Box<dyn Error>> {
    if path.exists() && !allow_overwrite {
        return Err(format!(
            "refusing to overwrite {}; pass --allow-overwrite",
            path.display()
        )
        .into());
    }
    let payload = json::to_json_pretty(report)?;
    fs::write(path, payload)?;
    Ok(())
}

fn write_markdown(
    report: &SloReport,
    path: &Path,
    allow_overwrite: bool,
) -> Result<(), Box<dyn Error>> {
    if path.exists() && !allow_overwrite {
        return Err(format!(
            "refusing to overwrite {}; pass --allow-overwrite",
            path.display()
        )
        .into());
    }
    let mut out = String::new();
    out.push_str("# Iroha 3 SLO Harness\n\n");
    out.push_str(&format!(
        "- Timestamp: {}\n- Git hash: {}\n- Iterations: {}\n- Sample size: {}\n\n",
        report.timestamp,
        report.git_hash.clone().unwrap_or_else(|| "unknown".into()),
        report.iterations,
        report.sample_size
    ));
    out.push_str("| SLO | Scenario | Objective (ms) | Observed (ms) | Status | Budget Ratio |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for outcome in &report.outcomes {
        let observed = outcome
            .observed_ms
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "n/a".into());
        let ratio = outcome
            .budget_ratio
            .map(|v| format!("{v:.2}x"))
            .unwrap_or_else(|| "-".into());
        out.push_str(&format!(
            "| {} | {} | {:.2} | {} | {} | {} |\n",
            outcome.label, outcome.scenario, outcome.objective_ms, observed, outcome.status, ratio
        ));
    }
    fs::write(path, out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> I3BenchReport {
        I3BenchReport {
            timestamp: "2026-11-20T00:00:00Z".to_string(),
            git_hash: Some("abc123".into()),
            config: crate::i3_bench_suite::BenchConfig {
                iterations: 4,
                sample_size: 2,
                flamegraph_hint: false,
            },
            scenarios: vec![
                crate::i3_bench_suite::ScenarioResult {
                    name: "fee_payer".into(),
                    nanos_per_iter: 40_000_000,
                    throughput_per_sec: 0.0,
                    allocations_per_iter: 0.0,
                    note: String::new(),
                },
                crate::i3_bench_suite::ScenarioResult {
                    name: "commit_cert_assembly".into(),
                    nanos_per_iter: 120_000_000,
                    throughput_per_sec: 0.0,
                    allocations_per_iter: 0.0,
                    note: String::new(),
                },
            ],
        }
    }

    #[test]
    fn evaluate_marks_pass_and_fail() {
        let budgets = SloBudgetFile {
            targets: vec![
                SloTarget {
                    id: "fee".into(),
                    label: Some("Fee path".into()),
                    scenario: "fee_payer".into(),
                    objective_ms: 50.0,
                    description: None,
                    burn_rate_fast: Some(14.4),
                    burn_rate_slow: Some(6.0),
                },
                SloTarget {
                    id: "finality".into(),
                    label: None,
                    scenario: "commit_cert_assembly".into(),
                    objective_ms: 80.0,
                    description: None,
                    burn_rate_fast: None,
                    burn_rate_slow: None,
                },
                SloTarget {
                    id: "missing".into(),
                    label: None,
                    scenario: "not_present".into(),
                    objective_ms: 10.0,
                    description: None,
                    burn_rate_fast: None,
                    burn_rate_slow: None,
                },
            ],
        };

        let report = evaluate_slos(&sample_report(), budgets);
        assert_eq!(report.outcomes.len(), 3);
        assert_eq!(report.outcomes[0].status, "pass");
        assert_eq!(report.outcomes[1].status, "fail");
        assert_eq!(report.outcomes[2].status, "missing");
    }
}
