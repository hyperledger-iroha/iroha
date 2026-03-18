use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Context, Result, eyre};
use norito::json::{Map, Value};
use time::OffsetDateTime;

use crate::JsonTarget;

const DEFAULT_PREFIX: &str = "203.0.113.0/24";
const DEFAULT_TRUSTLESS_SERVICE: &str = "soranet-trustless-verifier.service";
const DEFAULT_RESOLVER_SERVICE: &str = "soradns-resolver.service";
const DEFAULT_HOLD_SECONDS: u64 = 120;

struct Scenario {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    detection: &'static [&'static str],
    success: &'static [&'static str],
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        id: "prefix-withdrawal",
        title: "BGP prefix withdrawal",
        description: "Withdraw a customer prefix from one PoP to validate alerting and rollout playbooks.",
        detection: &[
            "BGP session/route alerts fire (gameday dashboard) and adoption smoke tests fail fast.",
            "`soranet_gw_*` error-rate/latency panels show the expected spike for the withdrawn prefix.",
        ],
        success: &[
            "Prefix is restored and gateway health returns under the SLO within the drill window.",
            "Drill log captures alert ids, detection time, and remediation owner.",
        ],
    },
    Scenario {
        id: "trustless-verifier-failure",
        title: "Trustless verifier outage",
        description: "Stop the trustless verifier to force gateway fallbacks and test GAR evidence capture.",
        detection: &[
            "Verifier health probes fail and moderation/proof-required fetches surface fallbacks.",
            "Honey/audit probes fail with explicit proof errors before cache serve-stale kicks in.",
        ],
        success: &[
            "Verifier service restarts cleanly, backlog drains, and alerts auto-resolve.",
            "GAR evidence and drill log record the outage window and recovery time.",
        ],
    },
    Scenario {
        id: "resolver-brownout",
        title: "Resolver brownout",
        description: "Throttle a SoraDNS resolver to validate DNS latency/timeout alerts and gateway failover.",
        detection: &[
            "Resolver latency/error budget alerts fire and client smoke tests time out.",
            "Gateway header audit reveals fallback cache headers while the resolver is degraded.",
        ],
        success: &[
            "Resolver latency returns under SLO, cache hit-rate stabilises, and alerts clear.",
            "Drill log captures observed impact and remediation steps for the brownout.",
        ],
    },
];

#[derive(Clone, Debug)]
pub struct ChaosKitOptions {
    pub output_dir: PathBuf,
    pub pop_label: String,
    pub gateway_host: String,
    pub resolver_host: String,
    pub quarter_label: Option<String>,
    pub now: Option<SystemTime>,
}

#[derive(Clone, Debug)]
pub struct ChaosKitOutcome {
    pub plan_json: PathBuf,
    pub plan_markdown: PathBuf,
    pub log_path: PathBuf,
    pub scripts: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ChaosReportOptions {
    pub log_path: PathBuf,
    pub output: JsonTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChaosScenarioSummary {
    pub id: String,
    pub injects: usize,
    pub detects: usize,
    pub recovers: usize,
    pub detection_ms: Option<u64>,
    pub recovery_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChaosSummary {
    pub generated_unix_ms: u64,
    pub log_path: PathBuf,
    pub scenarios: Vec<ChaosScenarioSummary>,
    pub max_detection_ms: Option<u64>,
    pub max_recovery_ms: Option<u64>,
}

impl ChaosSummary {
    pub fn to_value(&self) -> Value {
        let scenario_values = self
            .scenarios
            .iter()
            .map(|summary| {
                value_object([
                    ("id", Value::from(summary.id.clone())),
                    ("injects", Value::from(summary.injects as u64)),
                    ("detects", Value::from(summary.detects as u64)),
                    ("recovers", Value::from(summary.recovers as u64)),
                    (
                        "detection_ms",
                        summary.detection_ms.map(Value::from).unwrap_or(Value::Null),
                    ),
                    (
                        "recovery_ms",
                        summary.recovery_ms.map(Value::from).unwrap_or(Value::Null),
                    ),
                ])
            })
            .collect();

        value_object([
            ("generated_unix_ms", Value::from(self.generated_unix_ms)),
            ("log_path", Value::from(self.log_path.display().to_string())),
            ("scenarios", Value::Array(scenario_values)),
            (
                "max_detection_ms",
                self.max_detection_ms
                    .map(Value::from)
                    .unwrap_or(Value::Null),
            ),
            (
                "max_recovery_ms",
                self.max_recovery_ms.map(Value::from).unwrap_or(Value::Null),
            ),
        ])
    }
}

pub fn write_chaos_kit(options: ChaosKitOptions) -> Result<ChaosKitOutcome> {
    fs::create_dir_all(&options.output_dir).context("create chaos kit output directory")?;

    let now = options.now.unwrap_or_else(SystemTime::now);
    let quarter_label = options
        .quarter_label
        .clone()
        .unwrap_or_else(|| derive_quarter(now));
    let generated_ms = system_time_to_ms(now);
    let log_path = options.output_dir.join("chaos_events.ndjson");
    File::create(&log_path).context("create chaos log stub")?;

    let scenarios = build_scenarios(&options);
    let plan = value_object([
        ("generated_unix_ms", Value::from(generated_ms)),
        ("quarter", Value::from(quarter_label.clone())),
        (
            "targets",
            value_object([
                ("pop", Value::from(options.pop_label.clone())),
                ("gateway", Value::from(options.gateway_host.clone())),
                ("resolver", Value::from(options.resolver_host.clone())),
            ]),
        ),
        ("log_path", Value::from("chaos_events.ndjson")),
        ("scenarios", Value::Array(scenarios.clone())),
    ]);

    let plan_json = options.output_dir.join("plan.json");
    write_json_file(&plan_json, &plan)?;

    let plan_markdown = options.output_dir.join("plan.md");
    let plan_text = build_plan_markdown(&quarter_label, &options, &scenarios);
    fs::write(&plan_markdown, plan_text).context("write chaos plan markdown")?;

    let scripts = write_scripts(&options, &log_path)?;

    Ok(ChaosKitOutcome {
        plan_json,
        plan_markdown,
        log_path,
        scripts,
    })
}

pub fn summarize_log(options: ChaosReportOptions) -> Result<ChaosSummary> {
    let events = read_events(&options.log_path)?;
    let mut per_scenario: BTreeMap<String, Vec<ChaosEvent>> = BTreeMap::new();
    for scenario in SCENARIOS {
        per_scenario.insert(scenario.id.to_string(), Vec::new());
    }
    for event in events {
        per_scenario
            .entry(event.scenario.clone())
            .or_default()
            .push(event);
    }

    let mut summaries = Vec::new();
    let mut max_detection = None;
    let mut max_recovery = None;

    for (id, mut events) in per_scenario {
        events.sort_by_key(|event| event.ts_ms);
        let injects = events.iter().filter(|e| e.action == "inject").count();
        let detects = events.iter().filter(|e| e.action == "detect").count();
        let recovers = events.iter().filter(|e| e.action == "recover").count();

        let detection_ms = first_latency(&events, "inject", "detect");
        let recovery_ms = first_latency(&events, "inject", "recover");

        if let Some(value) = detection_ms {
            max_detection = Some(max_detection.unwrap_or(value).max(value));
        }
        if let Some(value) = recovery_ms {
            max_recovery = Some(max_recovery.unwrap_or(value).max(value));
        }

        summaries.push(ChaosScenarioSummary {
            id,
            injects,
            detects,
            recovers,
            detection_ms,
            recovery_ms,
        });
    }

    let summary = ChaosSummary {
        generated_unix_ms: system_time_to_ms(SystemTime::now()),
        log_path: options.log_path.clone(),
        scenarios: summaries,
        max_detection_ms: max_detection,
        max_recovery_ms: max_recovery,
    };
    write_json_target(options.output, &summary.to_value())?;
    Ok(summary)
}

struct ChaosEvent {
    ts_ms: u64,
    scenario: String,
    action: String,
}

fn build_scenarios(options: &ChaosKitOptions) -> Vec<Value> {
    SCENARIOS
        .iter()
        .map(|scenario| {
            let target = match scenario.id {
                "prefix-withdrawal" => options.pop_label.clone(),
                "trustless-verifier-failure" => options.gateway_host.clone(),
                "resolver-brownout" => options.resolver_host.clone(),
                _ => String::new(),
            };
            value_object([
                ("id", Value::from(scenario.id)),
                ("title", Value::from(scenario.title)),
                ("target", Value::from(target)),
                ("script", Value::from(script_name(scenario.id))),
                ("description", Value::from(scenario.description)),
                (
                    "detection",
                    Value::Array(
                        scenario
                            .detection
                            .iter()
                            .map(|line| Value::from(line.to_string()))
                            .collect(),
                    ),
                ),
                (
                    "success",
                    Value::Array(
                        scenario
                            .success
                            .iter()
                            .map(|line| Value::from(line.to_string()))
                            .collect(),
                    ),
                ),
            ])
        })
        .collect()
}

fn build_plan_markdown(
    quarter_label: &str,
    options: &ChaosKitOptions,
    scenarios: &[Value],
) -> String {
    let mut text = String::new();
    text.push_str(&format!(
        "# SoraNet Chaos GameDay ({quarter_label})\n\
\n\
This bundle exercises prefix withdrawal, trustless verifier failure, and resolver brownout drills\n\
for the SoraGlobal Gateway CDN. Generated with `cargo xtask soranet-chaos-kit`, it includes\n\
scripts, a shared event log, and a summary template so SRE can replay the drills quarterly\n\
without hand-editing files.\n\
\n\
- Pop: `{}`\n- Gateway host: `{}`\n- Resolver host: `{}`\n- Event log: `chaos_events.ndjson`\n\
\n\
## How to run a scenario\n\
1. Run from this directory with working SSH access to the target hosts.\n\
2. Run a scenario script with `APPLY=1` to perform the fault injection:\n\
   - `APPLY=1 CHAOS_HOLD_SECONDS=120 ./scripts/prefix_withdrawal.sh`\n\
   - `APPLY=1 ./scripts/trustless_verifier_failure.sh`\n\
   - `APPLY=1 CHAOS_HOLD_SECONDS=90 ./scripts/resolver_brownout.sh`\n\
   Scripts default to safe no-op mode when `APPLY` is unset so dry-runs remain deterministic.\n\
3. Capture detection/recovery timestamps with `./scripts/log_event.sh <scenario> detect|recover \"note\"`.\n\
4. Generate a summary: `cargo xtask soranet-chaos-report --log chaos_events.ndjson --out chaos_summary.json`.\n\
\n\
## Scenarios\n",
        options.pop_label, options.gateway_host, options.resolver_host
    ));

    for scenario in scenarios {
        let Some(id) = scenario.get("id").and_then(Value::as_str) else {
            continue;
        };
        let title = scenario
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let description = scenario
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let script = scenario
            .get("script")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let detection = scenario
            .get("detection")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let success = scenario
            .get("success")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        text.push_str(&format!(
            "### {title} (`{id}`)\n\n{description}\n\n- Script: `scripts/{script}`\n- Detection signals:\n"
        ));
        for line in detection {
            if let Some(value) = line.as_str() {
                text.push_str(&format!("  - {value}\n"));
            }
        }
        text.push_str("- Success criteria:\n");
        for line in success {
            if let Some(value) = line.as_str() {
                text.push_str(&format!("  - {value}\n"));
            }
        }
        text.push('\n');
    }

    text
}

fn write_scripts(options: &ChaosKitOptions, log_path: &Path) -> Result<Vec<PathBuf>> {
    let scripts_dir = options.output_dir.join("scripts");
    fs::create_dir_all(&scripts_dir).context("create chaos scripts directory")?;

    let mut scripts = Vec::new();
    let prefix_script = scripts_dir.join(script_name("prefix-withdrawal"));
    write_script(
        &prefix_script,
        &prefix_withdrawal_script(&scripts_dir, log_path, options),
    )?;
    scripts.push(prefix_script);

    let verifier_script = scripts_dir.join(script_name("trustless-verifier-failure"));
    write_script(
        &verifier_script,
        &trustless_verifier_script(&scripts_dir, log_path, options),
    )?;
    scripts.push(verifier_script);

    let resolver_script = scripts_dir.join(script_name("resolver-brownout"));
    write_script(
        &resolver_script,
        &resolver_brownout_script(&scripts_dir, log_path, options),
    )?;
    scripts.push(resolver_script);

    let logger_script = scripts_dir.join("log_event.sh");
    write_script(&logger_script, &log_event_script(&scripts_dir, log_path))?;
    scripts.push(logger_script);

    Ok(scripts)
}

fn prefix_withdrawal_script(
    scripts_dir: &Path,
    log_path: &Path,
    options: &ChaosKitOptions,
) -> String {
    format!(
        "#!/usr/bin/env bash
set -euo pipefail

SCENARIO=\"prefix-withdrawal\"
LOG_PATH=\"${{CHAOS_LOG:-{}}}\"
POP_LABEL=\"${{POP_LABEL:-{}}}\"
PREFIX=\"${{CHAOS_PREFIX:-{DEFAULT_PREFIX}}}\"
BGP_AS=\"${{CHAOS_BGP_AS:-64512}}\"
SSH_BIN=\"${{CHAOS_SSH:-ssh}}\"
HOLD_SECONDS=\"${{CHAOS_HOLD_SECONDS:-{DEFAULT_HOLD_SECONDS}}}\"

mkdir -p \"$(dirname \"$LOG_PATH\")\"
touch \"$LOG_PATH\"

log_event() {{
  local action=\"$1\"
  local note=\"$2\"
  local ts_ms=$(( $(date +%s) * 1000 ))
  printf '{{\"ts_ms\":%s,\"scenario\":\"%s\",\"action\":\"%s\",\"note\":\"%s\"}}\\n' \"$ts_ms\" \"$SCENARIO\" \"$action\" \"$note\" >> \"$LOG_PATH\"
}}

log_event \"inject\" \"withdrawing $PREFIX from $POP_LABEL\"
echo \"[soranet-chaos] Withdrawing $PREFIX from $POP_LABEL (APPLY=${{APPLY:-0}}, hold=$HOLD_SECONDS seconds)\"
if [[ \"${{APPLY:-0}}\" == \"1\" ]]; then
  $SSH_BIN \"$POP_LABEL\" \"sudo vtysh -c 'configure terminal' -c 'router bgp $BGP_AS' -c 'no network $PREFIX'\"
fi

sleep \"$HOLD_SECONDS\"

echo \"[soranet-chaos] Restoring $PREFIX on $POP_LABEL\"
if [[ \"${{APPLY:-0}}\" == \"1\" ]]; then
  $SSH_BIN \"$POP_LABEL\" \"sudo vtysh -c 'configure terminal' -c 'router bgp $BGP_AS' -c 'network $PREFIX'\"
fi
log_event \"recover\" \"restored $PREFIX on $POP_LABEL\"

./{}/log_event.sh \"$SCENARIO\" detect \"record detection time\" || true
./{}/log_event.sh \"$SCENARIO\" recover \"record recovery confirmation\" || true
",
        log_path.display(),
        options.pop_label,
        scripts_dir.file_name().unwrap_or_default().to_string_lossy(),
        scripts_dir.file_name().unwrap_or_default().to_string_lossy(),
    )
}

fn trustless_verifier_script(
    scripts_dir: &Path,
    log_path: &Path,
    options: &ChaosKitOptions,
) -> String {
    format!(
        "#!/usr/bin/env bash
set -euo pipefail

SCENARIO=\"trustless-verifier-failure\"
LOG_PATH=\"${{CHAOS_LOG:-{}}}\"
GATEWAY_HOST=\"${{CHAOS_GATEWAY_HOST:-{}}}\"
SERVICE=\"${{CHAOS_VERIFIER_SERVICE:-{DEFAULT_TRUSTLESS_SERVICE}}}\"
SSH_BIN=\"${{CHAOS_SSH:-ssh}}\"
HOLD_SECONDS=\"${{CHAOS_HOLD_SECONDS:-{DEFAULT_HOLD_SECONDS}}}\"

mkdir -p \"$(dirname \"$LOG_PATH\")\"
touch \"$LOG_PATH\"

log_event() {{
  local action=\"$1\"
  local note=\"$2\"
  local ts_ms=$(( $(date +%s) * 1000 ))
  printf '{{\"ts_ms\":%s,\"scenario\":\"%s\",\"action\":\"%s\",\"note\":\"%s\"}}\\n' \"$ts_ms\" \"$SCENARIO\" \"$action\" \"$note\" >> \"$LOG_PATH\"
}}

log_event \"inject\" \"stopping $SERVICE on $GATEWAY_HOST\"
echo \"[soranet-chaos] Stopping $SERVICE on $GATEWAY_HOST (APPLY=${{APPLY:-0}}, hold=$HOLD_SECONDS seconds)\"
if [[ \"${{APPLY:-0}}\" == \"1\" ]]; then
  $SSH_BIN \"$GATEWAY_HOST\" \"sudo systemctl stop $SERVICE\"
fi

sleep \"$HOLD_SECONDS\"

echo \"[soranet-chaos] Starting $SERVICE on $GATEWAY_HOST\"
if [[ \"${{APPLY:-0}}\" == \"1\" ]]; then
  $SSH_BIN \"$GATEWAY_HOST\" \"sudo systemctl start $SERVICE\"
fi
log_event \"recover\" \"started $SERVICE on $GATEWAY_HOST\"

./{}/log_event.sh \"$SCENARIO\" detect \"capture proof/alert fire\" || true
./{}/log_event.sh \"$SCENARIO\" recover \"capture backlog drain\" || true
",
        log_path.display(),
        options.gateway_host,
        scripts_dir.file_name().unwrap_or_default().to_string_lossy(),
        scripts_dir.file_name().unwrap_or_default().to_string_lossy(),
    )
}

fn resolver_brownout_script(
    scripts_dir: &Path,
    log_path: &Path,
    options: &ChaosKitOptions,
) -> String {
    format!(
        "#!/usr/bin/env bash
set -euo pipefail

SCENARIO=\"resolver-brownout\"
LOG_PATH=\"${{CHAOS_LOG:-{}}}\"
RESOLVER_HOST=\"${{CHAOS_RESOLVER_HOST:-{}}}\"
SERVICE=\"${{CHAOS_RESOLVER_SERVICE:-{DEFAULT_RESOLVER_SERVICE}}}\"
SSH_BIN=\"${{CHAOS_SSH:-ssh}}\"
HOLD_SECONDS=\"${{CHAOS_HOLD_SECONDS:-90}}\"  # shorter brownout by default

mkdir -p \"$(dirname \"$LOG_PATH\")\"
touch \"$LOG_PATH\"

log_event() {{
  local action=\"$1\"
  local note=\"$2\"
  local ts_ms=$(( $(date +%s) * 1000 ))
  printf '{{\"ts_ms\":%s,\"scenario\":\"%s\",\"action\":\"%s\",\"note\":\"%s\"}}\\n' \"$ts_ms\" \"$SCENARIO\" \"$action\" \"$note\" >> \"$LOG_PATH\"
}}

log_event \"inject\" \"throttling resolver $SERVICE on $RESOLVER_HOST\"
echo \"[soranet-chaos] Pausing $SERVICE on $RESOLVER_HOST (APPLY=${{APPLY:-0}}, hold=$HOLD_SECONDS seconds)\"
if [[ \"${{APPLY:-0}}\" == \"1\" ]]; then
  $SSH_BIN \"$RESOLVER_HOST\" \"sudo systemctl stop $SERVICE\"
fi

sleep \"$HOLD_SECONDS\"

echo \"[soranet-chaos] Restoring $SERVICE on $RESOLVER_HOST\"
if [[ \"${{APPLY:-0}}\" == \"1\" ]]; then
  $SSH_BIN \"$RESOLVER_HOST\" \"sudo systemctl start $SERVICE\"
fi
log_event \"recover\" \"restored resolver $SERVICE on $RESOLVER_HOST\"

./{}/log_event.sh \"$SCENARIO\" detect \"capture DNS alert\" || true
./{}/log_event.sh \"$SCENARIO\" recover \"capture latency recovery\" || true
",
        log_path.display(),
        options.resolver_host,
        scripts_dir.file_name().unwrap_or_default().to_string_lossy(),
        scripts_dir.file_name().unwrap_or_default().to_string_lossy(),
    )
}

fn log_event_script(_scripts_dir: &Path, log_path: &Path) -> String {
    format!(
        "#!/usr/bin/env bash
set -euo pipefail

if [[ \"$#\" -lt 2 ]]; then
  echo \"usage: $0 <scenario> <action> [note]\" 1>&2
  exit 1
fi

SCENARIO=\"$1\"
ACTION=\"$2\"
shift 2
NOTE=\"${{*:-}}\"  # optional note

LOG_PATH=\"${{CHAOS_LOG:-{}}}\"
mkdir -p \"$(dirname \"$LOG_PATH\")\"
touch \"$LOG_PATH\"

ts_ms=$(( $(date +%s) * 1000 ))
printf '{{\"ts_ms\":%s,\"scenario\":\"%s\",\"action\":\"%s\",\"note\":\"%s\"}}\\n' \"$ts_ms\" \"$SCENARIO\" \"$ACTION\" \"$NOTE\" >> \"$LOG_PATH\"
echo \"[soranet-chaos] recorded $ACTION for $SCENARIO -> $LOG_PATH\"
",
        log_path.display()
    )
}

fn write_script(path: &Path, contents: &str) -> Result<()> {
    let mut file =
        File::create(path).with_context(|| format!("create script {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("write script {}", path.display()))?;
    let perms = fs::Permissions::from_mode(0o750);
    fs::set_permissions(path, perms)
        .with_context(|| format!("set permissions on {}", path.display()))?;
    Ok(())
}

fn script_name(id: &str) -> String {
    match id {
        "prefix-withdrawal" => "prefix_withdrawal.sh".to_string(),
        "trustless-verifier-failure" => "trustless_verifier_failure.sh".to_string(),
        "resolver-brownout" => "resolver_brownout.sh".to_string(),
        other => format!("{other}.sh"),
    }
}

fn derive_quarter(now: SystemTime) -> String {
    let datetime = OffsetDateTime::from(now);
    let quarter = (datetime.month() as u8 - 1) / 3 + 1;
    format!("{}-Q{}", datetime.year(), quarter)
}

fn system_time_to_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn read_events(log_path: &Path) -> Result<Vec<ChaosEvent>> {
    let file = File::open(log_path).with_context(|| format!("open log {}", log_path.display()))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            norito::json::from_str(&line).with_context(|| format!("parse log line `{line}`"))?;
        let event = ChaosEvent {
            ts_ms: value
                .get("ts_ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| eyre!("missing ts_ms in log entry"))?,
            scenario: value
                .get("scenario")
                .and_then(Value::as_str)
                .ok_or_else(|| eyre!("missing scenario in log entry"))?
                .to_string(),
            action: value
                .get("action")
                .and_then(Value::as_str)
                .ok_or_else(|| eyre!("missing action in log entry"))?
                .to_string(),
        };
        events.push(event);
    }
    Ok(events)
}

fn first_latency(events: &[ChaosEvent], start: &str, end: &str) -> Option<u64> {
    let start_ts = events.iter().find(|event| event.action == start)?.ts_ms;
    let end_ts = events
        .iter()
        .find(|event| event.action == end && event.ts_ms >= start_ts)?
        .ts_ms;
    end_ts.checked_sub(start_ts)
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    norito::json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn write_json_target(target: JsonTarget, value: &Value) -> Result<()> {
    let text = norito::json::to_string_pretty(value)? + "\n";
    match target {
        JsonTarget::Stdout => {
            print!("{text}");
        }
        JsonTarget::File(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, text)?;
        }
    }
    Ok(())
}

fn value_object<I, K>(entries: I) -> Value
where
    I: IntoIterator<Item = (K, Value)>,
    K: Into<String>,
{
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.into(), value);
    }
    Value::Object(map)
}
