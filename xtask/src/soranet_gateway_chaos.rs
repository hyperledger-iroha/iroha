//! Chaos drill harness for the SoraGlobal Gateway CDN (SNNet-15F1).
//! Generates scenario packs, quarterly schedules, and runs dry-run or
//! execution passes to capture evidence bundles for SRE/GameDay drills.

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use eyre::{Result, WrapErr, eyre};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct ChaosScenario {
    pub id: String,
    pub title: String,
    pub description: String,
    pub alert: String,
    pub benchmarks: ScenarioBenchmarks,
    pub inject: Vec<ActionStep>,
    pub verify: Vec<ActionStep>,
    pub remediate: Vec<ActionStep>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct ScenarioBenchmarks {
    pub alert_budget_seconds: u64,
    pub recovery_budget_seconds: u64,
    pub max_backlog_percent: u64,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct ActionStep {
    pub name: String,
    pub command: Vec<String>,
    pub timeout_seconds: u64,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct ScheduleEntry {
    pub quarter: String,
    pub scenarios: Vec<String>,
    pub owner: String,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct ScenarioPack {
    pub version: u32,
    pub pop: String,
    pub scenarios: Vec<ChaosScenario>,
    pub schedule: Vec<ScheduleEntry>,
}

#[derive(Debug)]
pub struct ChaosAssets {
    pub scenarios_path: PathBuf,
    pub runbook_path: PathBuf,
    pub schedule_path: PathBuf,
}

#[derive(Debug)]
pub struct ChaosOptions {
    pub config_path: PathBuf,
    pub output_dir: PathBuf,
    pub pop: String,
    pub scenarios: ScenarioSelection,
    pub execute: bool,
    pub note: Option<String>,
    pub now: OffsetDateTime,
}

#[derive(Debug)]
pub enum ScenarioSelection {
    All,
    Only(Vec<String>),
}

#[derive(Debug)]
pub struct ChaosOutcome {
    pub plan_path: PathBuf,
    pub report_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[derive(Debug, JsonSerialize)]
struct PlanStep {
    stage: String,
    name: String,
    command: Vec<String>,
    timeout_seconds: u64,
    evidence: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct PlanScenario {
    id: String,
    title: String,
    alert: String,
    benchmarks: ScenarioBenchmarks,
    steps: Vec<PlanStep>,
}

#[derive(Debug, JsonSerialize)]
struct ChaosPlan {
    version: u32,
    run_id: String,
    pop: String,
    scenarios: Vec<PlanScenario>,
    execute: bool,
    note: Option<String>,
    started_at: String,
}

#[derive(Debug, JsonSerialize)]
struct StepReport {
    stage: String,
    name: String,
    command: Vec<String>,
    status: String,
    timeout_seconds: u64,
    duration_ms: u128,
    output: Option<String>,
    evidence: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct ScenarioReport {
    id: String,
    title: String,
    alert: String,
    benchmarks: ScenarioBenchmarks,
    steps: Vec<StepReport>,
}

#[derive(Debug, JsonSerialize)]
struct ChaosReport {
    run_id: String,
    pop: String,
    scenarios: Vec<ScenarioReport>,
    execute: bool,
    note: Option<String>,
    started_at: String,
}

/// Writes the default chaos scenario bundle, quarterly schedule, and runbook.
pub fn write_chaos_assets(
    output_dir: &Path,
    pop: &str,
    now: OffsetDateTime,
) -> Result<ChaosAssets> {
    fs::create_dir_all(output_dir).wrap_err_with(|| {
        format!(
            "failed to create chaos asset directory `{}`",
            output_dir.display()
        )
    })?;
    let pop_label = sanitize_label(pop);
    let scenarios_path = output_dir.join("chaos_scenarios.json");
    let runbook_path = output_dir.join("chaos_runbook.md");
    let schedule_path = output_dir.join("gameday_schedule.json");

    let scenario_pack = build_default_pack(&pop_label, now);
    let scenario_file = File::create(&scenarios_path)
        .wrap_err_with(|| format!("create {}", scenarios_path.display()))?;
    json::to_writer_pretty(scenario_file, &scenario_pack).wrap_err_with(|| {
        format!(
            "failed to write chaos scenario pack {}",
            scenarios_path.display()
        )
    })?;

    let runbook = render_runbook(&scenario_pack);
    fs::write(&runbook_path, runbook)
        .wrap_err_with(|| format!("write {}", runbook_path.display()))?;

    let schedule_file = File::create(&schedule_path)
        .wrap_err_with(|| format!("create {}", schedule_path.display()))?;
    json::to_writer_pretty(schedule_file, &scenario_pack.schedule).wrap_err_with(|| {
        format!(
            "failed to write GameDay schedule {}",
            schedule_path.display()
        )
    })?;

    Ok(ChaosAssets {
        scenarios_path,
        runbook_path,
        schedule_path,
    })
}

/// Execute or dry-run chaos scenarios, writing plan + report artifacts.
pub fn run(options: ChaosOptions) -> Result<ChaosOutcome> {
    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create chaos output directory `{}`",
            options.output_dir.display()
        )
    })?;
    let config_bytes = fs::read(&options.config_path).wrap_err_with(|| {
        format!(
            "failed to read chaos config `{}`",
            options.config_path.display()
        )
    })?;
    let pack: ScenarioPack =
        json::from_slice(&config_bytes).wrap_err("invalid chaos config JSON")?;
    let selected = select_scenarios(&pack.scenarios, &options.scenarios)?;

    let started_at = options
        .now
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let safe_ts = started_at.replace([':', '-', '+'], "").replace('T', "_");
    let run_id = format!("{}-{}", safe_ts, sanitize_label(&options.pop));
    let plan = build_plan(&selected, &run_id, &options, &started_at);
    let report = execute_plan(&selected, &options, &started_at, &run_id)?;

    let run_dir = options.output_dir.join(format!("run_{run_id}"));
    fs::create_dir_all(&run_dir).wrap_err_with(|| {
        format!(
            "failed to create chaos run directory `{}`",
            run_dir.display()
        )
    })?;

    let plan_path = run_dir.join("chaos_plan.json");
    let plan_file =
        File::create(&plan_path).wrap_err_with(|| format!("create {}", plan_path.display()))?;
    json::to_writer_pretty(plan_file, &plan)
        .wrap_err_with(|| format!("write chaos plan {}", plan_path.display()))?;

    let report_path = run_dir.join("chaos_report.json");
    let report_file =
        File::create(&report_path).wrap_err_with(|| format!("create {}", report_path.display()))?;
    json::to_writer_pretty(report_file, &report)
        .wrap_err_with(|| format!("write chaos report {}", report_path.display()))?;

    let markdown_path = run_dir.join("chaos_report.md");
    fs::write(&markdown_path, render_report_markdown(&report))
        .wrap_err_with(|| format!("write {}", markdown_path.display()))?;

    Ok(ChaosOutcome {
        plan_path,
        report_path,
        markdown_path,
    })
}

fn select_scenarios<'a>(
    scenarios: &'a [ChaosScenario],
    selection: &ScenarioSelection,
) -> Result<Vec<&'a ChaosScenario>> {
    match selection {
        ScenarioSelection::All => Ok(scenarios.iter().collect()),
        ScenarioSelection::Only(ids) => {
            let mut out = Vec::new();
            for id in ids {
                let Some(s) = scenarios.iter().find(|s| &s.id == id) else {
                    return Err(eyre!("unknown chaos scenario `{id}`"));
                };
                out.push(s);
            }
            Ok(out)
        }
    }
}

fn build_plan(
    scenarios: &[&ChaosScenario],
    run_id: &str,
    options: &ChaosOptions,
    started_at: &str,
) -> ChaosPlan {
    let plan_scenarios = scenarios
        .iter()
        .map(|scenario| PlanScenario {
            id: scenario.id.clone(),
            title: scenario.title.clone(),
            alert: scenario.alert.clone(),
            benchmarks: scenario.benchmarks.clone(),
            steps: collect_steps(scenario),
        })
        .collect();

    ChaosPlan {
        version: 1,
        run_id: run_id.to_string(),
        pop: options.pop.clone(),
        scenarios: plan_scenarios,
        execute: options.execute,
        note: options.note.clone(),
        started_at: started_at.to_string(),
    }
}

fn collect_steps(scenario: &ChaosScenario) -> Vec<PlanStep> {
    scenario
        .inject
        .iter()
        .map(|step| PlanStep {
            stage: "inject".to_string(),
            name: step.name.clone(),
            command: step.command.clone(),
            timeout_seconds: step.timeout_seconds,
            evidence: step.evidence.clone(),
        })
        .chain(scenario.verify.iter().map(|step| PlanStep {
            stage: "verify".to_string(),
            name: step.name.clone(),
            command: step.command.clone(),
            timeout_seconds: step.timeout_seconds,
            evidence: step.evidence.clone(),
        }))
        .chain(scenario.remediate.iter().map(|step| PlanStep {
            stage: "remediate".to_string(),
            name: step.name.clone(),
            command: step.command.clone(),
            timeout_seconds: step.timeout_seconds,
            evidence: step.evidence.clone(),
        }))
        .collect()
}

fn execute_plan(
    scenarios: &[&ChaosScenario],
    options: &ChaosOptions,
    started_at: &str,
    run_id: &str,
) -> Result<ChaosReport> {
    let mut scenario_reports = Vec::new();
    for scenario in scenarios {
        let mut step_reports = Vec::new();
        for (stage, step) in scenario
            .inject
            .iter()
            .map(|s| ("inject", s))
            .chain(scenario.verify.iter().map(|s| ("verify", s)))
            .chain(scenario.remediate.iter().map(|s| ("remediate", s)))
        {
            step_reports.push(run_step(stage, step, options.execute)?);
        }
        scenario_reports.push(ScenarioReport {
            id: scenario.id.clone(),
            title: scenario.title.clone(),
            alert: scenario.alert.clone(),
            benchmarks: scenario.benchmarks.clone(),
            steps: step_reports,
        });
    }

    Ok(ChaosReport {
        run_id: run_id.to_string(),
        pop: options.pop.clone(),
        scenarios: scenario_reports,
        execute: options.execute,
        note: options.note.clone(),
        started_at: started_at.to_string(),
    })
}

fn run_step(stage: &str, step: &ActionStep, execute: bool) -> Result<StepReport> {
    if step.command.is_empty() {
        return Err(eyre!("step `{}` is missing a command", step.name));
    }

    if !execute {
        return Ok(StepReport {
            stage: stage.to_string(),
            name: step.name.clone(),
            command: step.command.clone(),
            status: "skipped".to_string(),
            timeout_seconds: step.timeout_seconds,
            duration_ms: 0,
            output: None,
            evidence: step.evidence.clone(),
        });
    }

    let start = Instant::now();
    let mut cmd = Command::new(&step.command[0]);
    if step.command.len() > 1 {
        cmd.args(&step.command[1..]);
    }
    let output = cmd
        .output()
        .wrap_err_with(|| format!("failed to run chaos step `{}`", step.name))?;
    let duration_ms = start.elapsed().as_millis();
    let status_label = if !output.status.success() {
        "failed"
    } else if duration_ms > (step.timeout_seconds as u128 * 1000) {
        "timeout"
    } else {
        "ok"
    };
    let mut combined_output = String::new();
    if !output.stdout.is_empty() {
        combined_output.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        if !combined_output.is_empty() {
            combined_output.push('\n');
        }
        combined_output.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    Ok(StepReport {
        stage: stage.to_string(),
        name: step.name.clone(),
        command: step.command.clone(),
        status: status_label.to_string(),
        timeout_seconds: step.timeout_seconds,
        duration_ms,
        output: if combined_output.is_empty() {
            None
        } else {
            Some(combined_output)
        },
        evidence: step.evidence.clone(),
    })
}

fn build_default_pack(pop_label: &str, now: OffsetDateTime) -> ScenarioPack {
    let scenarios = vec![
        ChaosScenario {
            id: "prefix-withdrawal".to_string(),
            title: "Prefix withdrawal / failover".to_string(),
            description: "Withdraw one upstream prefix, validate alerting, and prove cache + resolver health before restore."
                .to_string(),
            alert: "BgpSessionFlap".to_string(),
            benchmarks: ScenarioBenchmarks {
                alert_budget_seconds: 300,
                recovery_budget_seconds: 900,
                max_backlog_percent: 10,
            },
            inject: vec![
                ActionStep {
                    name: "Withdraw primary prefix".to_string(),
                    command: vec!["echo".to_string(), format!("simulate prefix withdraw for {pop_label}")],
                    timeout_seconds: 120,
                    evidence: vec!["FRR drain timers observed".to_string()],
                },
                ActionStep {
                    name: "Observe scrape loss".to_string(),
                    command: vec!["echo".to_string(), "watch GatewayScrapeMissing alert fire".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Alertmanager entry captured".to_string()],
                },
            ],
            verify: vec![
                ActionStep {
                    name: "Verify cache hit floor".to_string(),
                    command: vec!["echo".to_string(), "ensure cache hit rate floor stays above 0.89".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Grafana cache hit panel screenshot".to_string()],
                },
                ActionStep {
                    name: "Check backlog gauges".to_string(),
                    command: vec!["echo".to_string(), "review hedging/backlog gauges for spillover".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Backlog gauge snapshot".to_string()],
                },
            ],
            remediate: vec![
                ActionStep {
                    name: "Restore prefix".to_string(),
                    command: vec!["echo".to_string(), "restore primary prefix and clear drains".to_string()],
                    timeout_seconds: 120,
                    evidence: vec!["FRR route state restored".to_string()],
                },
                ActionStep {
                    name: "Confirm alert recovery".to_string(),
                    command: vec!["echo".to_string(), "ensure GatewayScrapeMissing clears".to_string()],
                    timeout_seconds: 120,
                    evidence: vec!["Alert resolved with timestamp".to_string()],
                },
            ],
        },
        ChaosScenario {
            id: "trustless-verifier-failure".to_string(),
            title: "Trustless verifier stalled pipeline".to_string(),
            description: "Stop the trustless verifier path to force cache binding failures and prove quarantine + alerting."
                .to_string(),
            alert: "GatewayVerifierStall".to_string(),
            benchmarks: ScenarioBenchmarks {
                alert_budget_seconds: 240,
                recovery_budget_seconds: 600,
                max_backlog_percent: 5,
            },
            inject: vec![
                ActionStep {
                    name: "Pause verifier process".to_string(),
                    command: vec!["echo".to_string(), "pause verifier service to trigger stall".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Verifier pause command issued".to_string()],
                },
                ActionStep {
                    name: "Send probe".to_string(),
                    command: vec!["echo".to_string(), "run probe fetch against gateway".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Probe request id saved".to_string()],
                },
            ],
            verify: vec![
                ActionStep {
                    name: "Check cache binding alerts".to_string(),
                    command: vec!["echo".to_string(), "confirm moderation/cache-version alerts raised".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Alert sample recorded".to_string()],
                },
                ActionStep {
                    name: "Inspect quarantine counters".to_string(),
                    command: vec!["echo".to_string(), "inspect quarantine counters for stalled receipts".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Quarantine counter snapshot".to_string()],
                },
            ],
            remediate: vec![
                ActionStep {
                    name: "Resume verifier".to_string(),
                    command: vec!["echo".to_string(), "resume verifier service and replay receipts".to_string()],
                    timeout_seconds: 120,
                    evidence: vec!["Verifier resume log excerpt".to_string()],
                },
                ActionStep {
                    name: "Validate backpressure cleared".to_string(),
                    command: vec!["echo".to_string(), "confirm backlog gauges below 5%".to_string()],
                    timeout_seconds: 120,
                    evidence: vec!["Backlog cleared snapshot".to_string()],
                },
            ],
        },
        ChaosScenario {
            id: "resolver-brownout".to_string(),
            title: "Resolver RAD sync brownout".to_string(),
            description: "Throttle resolver RAD sync to prove ResolverProofStale alerting and GAR rollback steps."
                .to_string(),
            alert: "ResolverProofStale".to_string(),
            benchmarks: ScenarioBenchmarks {
                alert_budget_seconds: 300,
                recovery_budget_seconds: 600,
                max_backlog_percent: 8,
            },
            inject: vec![
                ActionStep {
                    name: "Throttle RAD sync".to_string(),
                    command: vec!["echo".to_string(), "simulate RAD sync throttle for resolver".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["RAD sync throttle applied".to_string()],
                },
                ActionStep {
                    name: "Record proof age".to_string(),
                    command: vec!["echo".to_string(), "capture proof age before alert".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Proof age snapshot".to_string()],
                },
            ],
            verify: vec![
                ActionStep {
                    name: "Watch ResolverProofStale".to_string(),
                    command: vec!["echo".to_string(), "confirm ResolverProofStale alert triggers".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["Alertmanager screen grab".to_string()],
                },
                ActionStep {
                    name: "Validate GAR enforcement pause".to_string(),
                    command: vec!["echo".to_string(), "check GAR enforcement paused for stale proofs".to_string()],
                    timeout_seconds: 60,
                    evidence: vec!["GAR pause note".to_string()],
                },
            ],
            remediate: vec![
                ActionStep {
                    name: "Restore RAD sync".to_string(),
                    command: vec!["echo".to_string(), "restore RAD sync cadence".to_string()],
                    timeout_seconds: 120,
                    evidence: vec!["RAD sync restored log".to_string()],
                },
                ActionStep {
                    name: "Re-arm enforcement".to_string(),
                    command: vec!["echo".to_string(), "re-arm GAR enforcement and note proof ages".to_string()],
                    timeout_seconds: 120,
                    evidence: vec!["Enforcement resume note".to_string()],
                },
            ],
        },
    ];

    let schedule = build_schedule(now, &scenarios);
    ScenarioPack {
        version: 1,
        pop: pop_label.to_string(),
        scenarios,
        schedule,
    }
}

fn build_schedule(now: OffsetDateTime, scenarios: &[ChaosScenario]) -> Vec<ScheduleEntry> {
    let current_year = now.year();
    let current_quarter = ((now.month() as i32 - 1) / 3) + 1;
    let rotation = [
        scenarios[0].id.clone(),
        scenarios[1].id.clone(),
        scenarios[2].id.clone(),
        "rotation".to_string(),
    ];

    (0..4)
        .map(|idx| {
            let quarter_idx = current_quarter + idx;
            let year = current_year + (quarter_idx - 1) / 4;
            let quarter = ((quarter_idx - 1) % 4) + 1;
            let label = format!("{year}-Q{quarter}");
            ScheduleEntry {
                quarter: label,
                scenarios: if rotation[idx as usize] == "rotation" {
                    scenarios.iter().map(|s| s.id.clone()).collect()
                } else {
                    vec![rotation[idx as usize].clone()]
                },
                owner: "sre-oncall".to_string(),
            }
        })
        .collect()
}

fn render_runbook(pack: &ScenarioPack) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Gateway Chaos Runbook ({})\n\n", pack.pop));
    out.push_str("Use `cargo xtask soranet-gateway-chaos` to orchestrate drills. Default runs are dry-run; pass `--execute` to run the scripted steps.\n\n");
    out.push_str("## Scenarios\n");
    for scenario in &pack.scenarios {
        out.push_str(&format!(
            "- **{}** (alert: `{}`) — alert budget {}s, recovery budget {}s, backlog cap {}%.\n",
            scenario.title,
            scenario.alert,
            scenario.benchmarks.alert_budget_seconds,
            scenario.benchmarks.recovery_budget_seconds,
            scenario.benchmarks.max_backlog_percent
        ));
    }
    out.push_str("\n## Quarterly schedule\n");
    for entry in &pack.schedule {
        out.push_str(&format!(
            "- {} → scenarios: {} (owner: {})\n",
            entry.quarter,
            entry.scenarios.join(", "),
            entry.owner
        ));
    }
    out
}

fn render_report_markdown(report: &ChaosReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Gateway Chaos Report ({})\n\n", report.pop));
    out.push_str(&format!(
        "- Run: `{}`\n- Mode: {}\n- Started: {}\n",
        report.run_id,
        if report.execute { "execute" } else { "dry-run" },
        report.started_at
    ));
    if let Some(note) = &report.note {
        out.push_str(&format!("- Note: {note}\n"));
    }
    out.push('\n');

    for scenario in &report.scenarios {
        out.push_str(&format!("## {} (`{}`)\n\n", scenario.title, scenario.id));
        out.push_str(&format!(
            "Alert: `{}` · Alert budget: {}s · Recovery budget: {}s · Backlog cap: {}%\n\n",
            scenario.alert,
            scenario.benchmarks.alert_budget_seconds,
            scenario.benchmarks.recovery_budget_seconds,
            scenario.benchmarks.max_backlog_percent
        ));
        for step in &scenario.steps {
            out.push_str(&format!(
                "- [{}] {}: {} ({}s)\n",
                step.status, step.stage, step.name, step.timeout_seconds
            ));
            if let Some(output) = &step.output {
                out.push_str(&format!("  - output: {}\n", output.trim()));
            }
            if !step.evidence.is_empty() {
                out.push_str(&format!("  - evidence: {}\n", step.evidence.join(", ")));
            }
        }
        out.push('\n');
    }

    out
}

fn sanitize_label(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use norito::json::Value;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn default_assets_are_written() {
        let temp = tempdir().expect("tempdir");
        let now = OffsetDateTime::from_unix_timestamp(1_702_000_000).expect("timestamp");
        let assets = write_chaos_assets(temp.path(), "qa-pop_01", now).expect("write assets");

        let pack_bytes = fs::read(&assets.scenarios_path).expect("scenario file exists");
        let pack: ScenarioPack = json::from_slice(&pack_bytes).expect("scenario pack parses");
        assert_eq!(pack.scenarios.len(), 3);
        assert_eq!(pack.scenarios[0].id, "prefix-withdrawal");
        assert_eq!(pack.schedule.len(), 4);

        let runbook = fs::read_to_string(&assets.runbook_path).expect("runbook exists");
        assert!(
            runbook.contains("Gateway Chaos Runbook"),
            "runbook text should mention runbook"
        );
    }

    #[test]
    fn chaos_runner_generates_reports() {
        let temp = tempdir().expect("tempdir");
        let now = OffsetDateTime::from_unix_timestamp(1_702_000_000).expect("timestamp");
        let assets = write_chaos_assets(temp.path(), "qa-pop_01", now).expect("write assets");

        let run_out = temp.path().join("runs");
        let outcome = run(ChaosOptions {
            config_path: assets.scenarios_path.clone(),
            output_dir: run_out.clone(),
            pop: "qa-pop_01".to_string(),
            scenarios: ScenarioSelection::Only(vec!["prefix-withdrawal".to_string()]),
            execute: true,
            note: Some("test run".to_string()),
            now,
        })
        .expect("chaos run executes");

        assert!(outcome.plan_path.exists());
        assert!(outcome.report_path.exists());
        assert!(outcome.markdown_path.exists());

        let report_bytes = fs::read(outcome.report_path).expect("report exists");
        let report: Value = json::from_slice(&report_bytes).expect("report parses");
        assert_eq!(report["scenarios"].as_array().unwrap().len(), 1);
        assert_eq!(
            report["scenarios"][0]["steps"][0]["status"],
            Value::from("ok")
        );
    }
}
