//! SN15-M0 operations pack for the SoraGlobal Gateway CDN.
//! Emits observability, compliance, and security scaffolding so M0 PoPs
//! rehearse the full evidence bundle before production.

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr, eyre};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::{self, Value},
};
use time::OffsetDateTime;

use crate::soranet_gateway_chaos::{self, ChaosScenario, ScenarioPack, ScheduleEntry};

#[derive(Debug)]
pub struct GatewayOpsOptions {
    pub output_dir: PathBuf,
    pub pop: String,
}

#[derive(Debug)]
pub struct GatewayOpsMultiOptions {
    pub output_dir: PathBuf,
    pub pops: Vec<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct GatewayOpsOutcome {
    pub otel_config_path: PathBuf,
    pub dashboard_path: PathBuf,
    pub alert_rules_path: PathBuf,
    pub gameday_plan_path: PathBuf,
    pub chaos_plan_path: PathBuf,
    pub chaos_scenarios_path: PathBuf,
    pub chaos_runbook_path: PathBuf,
    pub gameday_schedule_path: PathBuf,
    pub chaos_metrics_path: PathBuf,
    pub chaos_injector_path: PathBuf,
    pub compliance_outline_path: PathBuf,
    pub security_baseline_path: PathBuf,
    pub pq_checklist_path: PathBuf,
    pub summary_path: PathBuf,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct GatewayOpsFederatedOutcome {
    pub per_pop: Vec<GatewayOpsOutcome>,
    pub federated_otel_config_path: PathBuf,
    pub gameday_rotation_path: PathBuf,
    pub gameday_rotation_markdown_path: PathBuf,
    pub summary_path: PathBuf,
}

#[derive(Debug, JsonSerialize)]
struct GatewayOpsSummary {
    pop: String,
    otel_config_path: String,
    dashboard_path: String,
    alert_rules_path: String,
    gameday_plan_path: String,
    chaos_plan_path: String,
    chaos_scenarios_path: String,
    chaos_runbook_path: String,
    gameday_schedule_path: String,
    chaos_metrics_path: String,
    chaos_injector_path: String,
    compliance_outline_path: String,
    security_baseline_path: String,
    pq_checklist_path: String,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, Clone, PartialEq, Eq)]
pub struct GamedayRotationEntry {
    pub pop: String,
    pub quarter: String,
    pub scenarios: Vec<String>,
    pub owner: String,
}

/// Write the SN15-M0 observability/compliance/security pack to disk.
pub fn write_gateway_ops_pack(options: GatewayOpsOptions) -> Result<GatewayOpsOutcome> {
    let GatewayOpsOptions { output_dir, pop } = options;
    fs::create_dir_all(&output_dir).wrap_err_with(|| {
        format!(
            "failed to create gateway ops output directory `{}`",
            output_dir.display()
        )
    })?;

    let otel_config_path = output_dir.join("otel_collector.yaml");
    let dashboard_path = output_dir.join("grafana_dashboard.json");
    let alert_rules_path = output_dir.join("alert_rules.yml");
    let gameday_plan_path = output_dir.join("gameday_plan.md");
    let chaos_plan_path = output_dir.join("chaos_plan.md");
    let chaos_metrics_path = output_dir.join("chaos_metrics.json");
    let chaos_injector_path = output_dir.join("chaos_injector.sh");
    let compliance_outline_path = output_dir.join("gar_compliance_outline.md");
    let security_baseline_path = output_dir.join("security_baseline.md");
    let pq_checklist_path = output_dir.join("pq_readiness_checklist.json");
    let summary_path = output_dir.join("ops_summary.json");

    let pop_label = sanitize_label(&pop);
    let chaos_assets = soranet_gateway_chaos::write_chaos_assets(
        &output_dir,
        &pop_label,
        OffsetDateTime::now_utc(),
    )?;
    let scenarios: Vec<ChaosScenario> = {
        let pack_bytes = fs::read(&chaos_assets.scenarios_path).wrap_err_with(|| {
            format!(
                "failed to read chaos scenarios {}",
                chaos_assets.scenarios_path.display()
            )
        })?;
        let pack: ScenarioPack =
            json::from_slice(&pack_bytes).wrap_err("failed to parse chaos scenario pack")?;
        pack.scenarios
    };
    let schedule: Vec<ScheduleEntry> = {
        let schedule_bytes = fs::read(&chaos_assets.schedule_path).wrap_err_with(|| {
            format!(
                "failed to read chaos schedule {}",
                chaos_assets.schedule_path.display()
            )
        })?;
        json::from_slice(&schedule_bytes).wrap_err("failed to parse chaos schedule")?
    };
    let chaos_scenarios_path = chaos_assets.scenarios_path.clone();
    let chaos_runbook_path = chaos_assets.runbook_path.clone();
    let gameday_schedule_path = chaos_assets.schedule_path.clone();

    fs::write(&otel_config_path, render_otel_config(&pop_label)).wrap_err_with(|| {
        format!(
            "failed to write OTEL collector config {}",
            otel_config_path.display()
        )
    })?;

    let dashboard_spec = render_dashboard(&pop_label);
    let dashboard_file = File::create(&dashboard_path)
        .wrap_err_with(|| format!("create {}", dashboard_path.display()))?;
    json::to_writer_pretty(dashboard_file, &dashboard_spec).wrap_err_with(|| {
        format!(
            "failed to write Grafana dashboard {}",
            dashboard_path.display()
        )
    })?;

    fs::write(&alert_rules_path, render_alert_rules(&pop_label))
        .wrap_err_with(|| format!("failed to write alert rules {}", alert_rules_path.display()))?;
    fs::write(&gameday_plan_path, render_gameday_plan(&pop_label)).wrap_err_with(|| {
        format!(
            "failed to write game day plan {}",
            gameday_plan_path.display()
        )
    })?;
    fs::write(
        &chaos_plan_path,
        render_chaos_plan(&pop_label, &scenarios, &schedule),
    )
    .wrap_err_with(|| format!("failed to write chaos plan {}", chaos_plan_path.display()))?;
    let chaos_metrics = render_chaos_metrics(&pop_label);
    let chaos_metrics_file = File::create(&chaos_metrics_path)
        .wrap_err_with(|| format!("create {}", chaos_metrics_path.display()))?;
    json::to_writer_pretty(chaos_metrics_file, &chaos_metrics).wrap_err_with(|| {
        format!(
            "failed to write chaos metrics {}",
            chaos_metrics_path.display()
        )
    })?;
    fs::write(
        &chaos_injector_path,
        render_chaos_injector(&pop_label, &scenarios),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write chaos injector {}",
            chaos_injector_path.display()
        )
    })?;
    fs::write(
        &compliance_outline_path,
        render_compliance_outline(&pop_label),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write compliance outline {}",
            compliance_outline_path.display()
        )
    })?;
    fs::write(
        &security_baseline_path,
        render_security_baseline(&pop_label),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write security baseline {}",
            security_baseline_path.display()
        )
    })?;

    let pq_checklist = render_pq_checklist(&pop_label);
    let pq_file = File::create(&pq_checklist_path)
        .wrap_err_with(|| format!("create {}", pq_checklist_path.display()))?;
    json::to_writer_pretty(pq_file, &pq_checklist).wrap_err_with(|| {
        format!(
            "failed to write PQ checklist {}",
            pq_checklist_path.display()
        )
    })?;

    let summary = GatewayOpsSummary {
        pop,
        otel_config_path: summarize_path(&otel_config_path, &output_dir),
        dashboard_path: summarize_path(&dashboard_path, &output_dir),
        alert_rules_path: summarize_path(&alert_rules_path, &output_dir),
        gameday_plan_path: summarize_path(&gameday_plan_path, &output_dir),
        chaos_plan_path: summarize_path(&chaos_plan_path, &output_dir),
        chaos_scenarios_path: summarize_path(&chaos_scenarios_path, &output_dir),
        chaos_runbook_path: summarize_path(&chaos_runbook_path, &output_dir),
        gameday_schedule_path: summarize_path(&gameday_schedule_path, &output_dir),
        chaos_metrics_path: summarize_path(&chaos_metrics_path, &output_dir),
        chaos_injector_path: summarize_path(&chaos_injector_path, &output_dir),
        compliance_outline_path: summarize_path(&compliance_outline_path, &output_dir),
        security_baseline_path: summarize_path(&security_baseline_path, &output_dir),
        pq_checklist_path: summarize_path(&pq_checklist_path, &output_dir),
    };
    let summary_file = File::create(&summary_path)
        .wrap_err_with(|| format!("create {}", summary_path.display()))?;
    json::to_writer_pretty(summary_file, &summary)
        .wrap_err_with(|| format!("failed to write ops summary {}", summary_path.display()))?;

    Ok(GatewayOpsOutcome {
        otel_config_path,
        dashboard_path,
        alert_rules_path,
        gameday_plan_path,
        chaos_plan_path,
        chaos_scenarios_path,
        chaos_runbook_path,
        gameday_schedule_path,
        chaos_metrics_path,
        chaos_injector_path,
        compliance_outline_path,
        security_baseline_path,
        pq_checklist_path,
        summary_path,
    })
}

/// Write the multi-PoP SN15-F observability pack for the alpha rollout (three PoPs).
pub fn write_gateway_ops_federated_pack(
    options: GatewayOpsMultiOptions,
) -> Result<GatewayOpsFederatedOutcome> {
    if options.pops.is_empty() {
        return Err(eyre!(
            "at least one pop must be provided for the federated ops pack"
        ));
    }

    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create federated ops output directory `{}`",
            options.output_dir.display()
        )
    })?;

    let mut outcomes = Vec::new();
    for pop in &options.pops {
        let pop_dir = options.output_dir.join(sanitize_label(pop));
        let outcome = write_gateway_ops_pack(GatewayOpsOptions {
            output_dir: pop_dir,
            pop: pop.clone(),
        })?;
        outcomes.push(outcome);
    }

    let federated_otel_config_path = options.output_dir.join("otel_federated.yaml");
    fs::write(
        &federated_otel_config_path,
        render_federated_otel_config(&options.pops),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write federated OTEL config {}",
            federated_otel_config_path.display()
        )
    })?;

    let rotation_path = options.output_dir.join("gameday_rotation.json");
    let rotation_markdown_path = options.output_dir.join("gameday_rotation.md");
    let rotation_entries = build_gameday_rotation(&options.pops, &outcomes)?;
    let rotation_file = File::create(&rotation_path)
        .wrap_err_with(|| format!("create {}", rotation_path.display()))?;
    json::to_writer_pretty(rotation_file, &rotation_entries).wrap_err_with(|| {
        format!(
            "failed to write federated GameDay rotation {}",
            rotation_path.display()
        )
    })?;
    fs::write(
        &rotation_markdown_path,
        render_gameday_rotation_markdown(&rotation_entries),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write federated GameDay rotation markdown {}",
            rotation_markdown_path.display()
        )
    })?;

    let summary_path = options.output_dir.join("ops_federated_summary.json");
    let mut summary = json::Map::new();
    summary.insert("pops".into(), norito::json!(options.pops));
    summary.insert(
        "federated_otel_config".into(),
        norito::json!(summarize_path(
            &federated_otel_config_path,
            &options.output_dir
        )),
    );
    summary.insert(
        "gameday_rotation_json".into(),
        norito::json!(summarize_path(&rotation_path, &options.output_dir)),
    );
    summary.insert(
        "gameday_rotation_markdown".into(),
        norito::json!(summarize_path(&rotation_markdown_path, &options.output_dir)),
    );
    let summary_file = File::create(&summary_path)
        .wrap_err_with(|| format!("create {}", summary_path.display()))?;
    json::to_writer_pretty(summary_file, &Value::Object(summary)).wrap_err_with(|| {
        format!(
            "failed to write federated ops summary {}",
            summary_path.display()
        )
    })?;

    Ok(GatewayOpsFederatedOutcome {
        per_pop: outcomes,
        federated_otel_config_path,
        gameday_rotation_path: rotation_path,
        gameday_rotation_markdown_path: rotation_markdown_path,
        summary_path,
    })
}

fn render_otel_config(pop_label: &str) -> String {
    let topic = format!("otel-{pop_label}");
    format!(
        "# OTEL collector profile for {pop_label} (SN15-M0-10)\n\
receivers:\n\
  otlp:\n\
    protocols:\n\
      grpc:\n\
        endpoint: 0.0.0.0:4317\n\
      http:\n\
        endpoint: 0.0.0.0:4318\n\
  prometheus:\n\
    config:\n\
      scrape_configs:\n\
        - job_name: gateway\n\
          scrape_interval: 10s\n\
          static_configs:\n\
            - targets: [\"127.0.0.1:19092\"]\n\
              labels:\n\
                pop: {pop_label}\n\
                role: gateway\n\
        - job_name: resolver\n\
          scrape_interval: 15s\n\
          static_configs:\n\
            - targets: [\"127.0.0.1:9100\"]\n\
              labels:\n\
                pop: {pop_label}\n\
                role: resolver\n\
  filelog:\n\
    include:\n\
      - /var/log/soranet/*.log\n\
    operators:\n\
      - type: add\n\
        field: attributes.pop\n\
        value: {pop_label}\n\
      - type: move\n\
        from: attributes[\"http.request.header.x-forwarded-for\"]\n\
        to: attributes.scrubbed_xff\n\
      - type: omit\n\
        field: attributes[\"http.request.header.authorization\"]\n\
processors:\n\
  batch:\n\
    timeout: 5s\n\
    send_batch_size: 1024\n\
  resourcedetection:\n\
    detectors: [system]\n\
  attributes/drop_pii:\n\
    actions:\n\
      - key: client.ip\n\
        action: delete\n\
      - key: http.request.header.cookie\n\
        action: delete\n\
  tailsampling:\n\
    decision_wait: 5s\n\
    num_traces: 50000\n\
    policies:\n\
      - name: error-budget\n\
        type: probabilistic\n\
        configuration:\n\
          sampling_percentage: 15\n\
      - name: cache-miss\n\
        type: string_attribute\n\
        configuration:\n\
          key: sorafs.cache.result\n\
          values: [\"miss\", \"stale\"]\n\
exporters:\n\
  kafka:\n\
    brokers: [\"redpanda:9092\"]\n\
    topic: {topic}\n\
  prometheusremotewrite:\n\
    endpoint: http://mimir:9009/api/v1/push\n\
  loki:\n\
    endpoint: http://loki:3100/loki/api/v1/push\n\
  clickhouse:\n\
    endpoint: tcp://clickhouse:9000\n\
service:\n\
  telemetry:\n\
    logs:\n\
      level: info\n\
  pipelines:\n\
    traces:\n\
      receivers: [otlp]\n\
      processors: [resourcedetection, tailsampling, batch]\n\
      exporters: [kafka]\n\
    metrics:\n\
      receivers: [prometheus]\n\
      processors: [batch]\n\
      exporters: [prometheusremotewrite]\n\
    logs:\n\
      receivers: [filelog]\n\
      processors: [attributes/drop_pii, batch]\n\
      exporters: [loki, clickhouse]\n\
"
    )
}

fn render_dashboard(pop_label: &str) -> Value {
    let payload = format!(
        r#"{{
  "title": "SoraGlobal Gateway M0 ({pop})",
  "tags": ["soranet", "m0", "{pop}"],
  "time": {{"from": "now-6h", "to": "now"}},
  "panels": [
    {{
      "id": 1,
      "type": "timeseries",
      "title": "Gateway latency p95",
      "targets": [{{"expr": "histogram_quantile(0.95, rate(sorafs_gateway_latency_ms_bucket{{pop=\"{pop}\"}}[5m]))", "legendFormat": "gateway latency p95"}}],
      "description": "Target 350ms p95 with 500ms error budget ceiling."
    }},
    {{
      "id": 2,
      "type": "stat",
      "title": "Cache hit rate",
      "targets": [{{"expr": "sum(rate(sorafs_gateway_cache_hits_total{{pop=\"{pop}\"}}[5m])) / sum(rate(sorafs_gateway_cache_requests_total{{pop=\"{pop}\"}}[5m]))"}}],
      "thresholds": {{
        "mode": "absolute",
        "steps": [
          {{"color": "green", "value": 0.0}},
          {{"color": "orange", "value": 0.92}},
          {{"color": "red", "value": 0.89}}
        ]
      }},
      "description": "Hold >92% hit rate during drills; annotate dips with resolver/gateway incidents."
    }},
    {{
      "id": 3,
      "type": "stat",
      "title": "Error budget burn (hourly)",
      "targets": [{{"expr": "(1 - avg_over_time(sorafs_gateway_success_ratio{{pop=\"{pop}\"}}[1h])) * 100"}}],
      "description": "Keeps SN15-M0 burn rate visible for change approvals."
    }},
    {{
      "id": 4,
      "type": "timeseries",
      "title": "Resolver proof age",
      "targets": [{{"expr": "max_over_time(soradns_rad_proof_age_seconds{{pop=\"{pop}\"}}[15m])"}}],
      "description": "Alarm before RAD proofs age past 15m; ties to GAR policy enforcement."
    }}
  ],
  "annotations": {{
    "list": [
      {{"name": "deploy", "enable": true, "expr": "changes(sorafs_gateway_deploy_revision{{pop=\"{pop}\"}}[5m]) > 0"}}
    ]
  }}
}}"#,
        pop = pop_label
    );
    json::from_str(&payload).expect("dashboard template valid")
}

fn render_federated_otel_config(pops: &[String]) -> String {
    let mut kafka_receivers = String::new();
    let mut trace_receivers = Vec::new();
    let mut gateway_targets = String::new();
    let mut resolver_targets = String::new();

    for pop in pops {
        let label = sanitize_label(pop);
        kafka_receivers.push_str(&format!(
            "  kafka/{label}:\n    brokers: [\"redpanda:9092\"]\n    topic: otel-{label}\n    encoding: otlp_proto\n"
        ));
        trace_receivers.push(format!("kafka/{label}"));
        gateway_targets.push_str(&format!(
            "            - targets: [\"{label}.gateway:19092\"]\n              labels:\n                pop: {label}\n                role: gateway\n"
        ));
        resolver_targets.push_str(&format!(
            "            - targets: [\"{label}.resolver:9100\"]\n              labels:\n                pop: {label}\n                role: resolver\n"
        ));
    }

    let trace_receivers = trace_receivers.join(", ");
    format!(
        "# Federated OTEL collector profile (SN15-F multi-PoP)\n\
receivers:\n\
{kafka_receivers}  prometheus:\n\
    config:\n\
      scrape_configs:\n\
        - job_name: gateway\n\
          scrape_interval: 15s\n\
          static_configs:\n\
{gateway_targets}        - job_name: resolver\n\
          scrape_interval: 20s\n\
          static_configs:\n\
{resolver_targets}processors:\n\
  batch:\n\
    timeout: 5s\n\
    send_batch_size: 1024\n\
exporters:\n\
  prometheusremotewrite:\n\
    endpoint: http://mimir:9009/api/v1/push\n\
  clickhouse:\n\
    endpoint: tcp://clickhouse:9000\n\
  loki:\n\
    endpoint: http://loki:3100/loki/api/v1/push\n\
service:\n\
  telemetry:\n\
    logs:\n\
      level: info\n\
  pipelines:\n\
    traces:\n\
      receivers: [{trace_receivers}]\n\
      processors: [batch]\n\
      exporters: [clickhouse]\n\
    metrics:\n\
      receivers: [prometheus]\n\
      processors: [batch]\n\
      exporters: [prometheusremotewrite]\n\
    logs:\n\
      receivers: [{trace_receivers}]\n\
      processors: [batch]\n\
      exporters: [loki, clickhouse]\n\
"
    )
}

fn render_alert_rules(pop_label: &str) -> String {
    format!(
        "# Prometheus alert rules for {pop_label} (SN15-M0-10/11)\n\
groups:\n\
- name: soranet-gateway-m0\n\
  rules:\n\
    - alert: GatewayScrapeMissing\n\
      expr: up{{job=\"gateway\",pop=\"{pop_label}\"}} == 0\n\
      for: 2m\n\
      labels:\n\
        severity: critical\n\
        pop: {pop_label}\n\
      annotations:\n\
        summary: \"Gateway scrape missing at {pop_label}\"\n\
        description: \"Prometheus cannot reach the gateway metrics endpoint; check PoP health and firewall.\"\n\
    - alert: CacheHitRateDrop\n\
      expr: (sum(rate(sorafs_gateway_cache_hits_total{{pop=\"{pop_label}\"}}[5m])) / sum(rate(sorafs_gateway_cache_requests_total{{pop=\"{pop_label}\"}}[5m]))) < 0.89\n\
      for: 10m\n\
      labels:\n\
        severity: warning\n\
        pop: {pop_label}\n\
      annotations:\n\
        summary: \"Cache hit rate below 89%\"\n\
        description: \"Investigate resolver health, origin latency, or cache eviction bursts before making the orchestrator default-on.\"\n\
    - alert: ResolverProofStale\n\
      expr: max_over_time(soradns_rad_proof_age_seconds{{pop=\"{pop_label}\"}}[15m]) > 900\n\
      for: 1m\n\
      labels:\n\
        severity: critical\n\
        pop: {pop_label}\n\
      annotations:\n\
        summary: \"RAD proof age exceeding 15 minutes\"\n\
        description: \"Refresh SoraDNS sync and verify GAR bindings; stale proofs block GAR enforcement checks.\"\n\
    - alert: BgpSessionFlap\n\
      expr: changes(soranet_bgp_session_up{{pop=\"{pop_label}\"}}[10m]) > 2\n\
      for: 0m\n\
      labels:\n\
        severity: warning\n\
        pop: {pop_label}\n\
      annotations:\n\
        summary: \"BGP session flapping\"\n\
        description: \"Coordinate with edge on-call to confirm drain/withdraw workflows and capture evidence for SN15-M0 drills.\"\n\
    - alert: TailSamplingBackpressure\n\
      expr: sum(rate(otel_tail_sample_dropped_spans{{pop=\"{pop_label}\"}}[5m])) > 0\n\
      for: 5m\n\
      labels:\n\
        severity: warning\n\
        pop: {pop_label}\n\
      annotations:\n\
        summary: \"Tail sampling is dropping spans\"\n\
        description: \"Tune sampling percentage or Kafka throughput; span loss invalidates change packets for observability GA.\"\n"
    )
}

fn render_gameday_plan(pop_label: &str) -> String {
    format!(
        "# SN15-M0 GameDay Scenarios ({pop_label})\n\
\n\
- **Quarterly cadence:** Follow `gameday_schedule.json` + `chaos_runbook.md` to rotate scenarios once per quarter (weekday + weekend). Store outputs under `artifacts/soranet/gateway_chaos/`.\n\
- **Harness:** `cargo xtask soranet-gateway-chaos --pop {pop_label} --config chaos_scenarios.json --out artifacts/soranet/gateway_chaos --scenario <id> [--execute] [--note <text>]`\n\
- **Scenarios:** Prefix withdrawal/failover, trustless verifier stall, resolver proof brownout (see `chaos_plan.md` + `chaos_runbook.md` for steps and budgets).\n\
- **Signals to watch:** BgpSessionFlap, ResolverProofStale, CacheHitRateDrop, TailSamplingBackpressure, chaos_metrics.json expressions.\n\
- **Evidence bundle:** Attach alert exports, Grafana screenshots, chaos_report.json/md from the harness, and any manual rollback notes to the GameDay invite.\n"
    )
}

fn render_compliance_outline(pop_label: &str) -> String {
    format!(
        "# GAR Enforcement Outline ({pop_label}, SN15-M0-11)\n\
\n\
- **Receipt schema:** Every purge/moderation/policy change emits `GarEnforcementReceiptV1` with fields `{{ action, manifest_cid, cache_version, reason, issued_at, issued_by, pop }}`. Persist JSON + Norito copies under `artifacts/soranet/gateway/{pop_label}/gar_receipts/`.\n\
- **Distribution:** Broadcast enforcement events on NATS subject `gar.enforce.{pop_label}` with at-least-once delivery; PoPs ACK by writing `gar_enforcement_ack.json` containing receipt hash + applied version.\n\
- **Audit trail:** Nightly job exports merged receipts + ACKs to ClickHouse table `gar_enforcement_log` keyed by `{pop_label}` and `manifest_cid`. Dashboards read from this table and alert on missing ACKs >15m.\n\
- **Honey tokens:** Reserve two synthetic manifests per sprint to exercise purge + moderation flows; record honey probe outcomes next to the receipts and attach to governance packets.\n\
- **Runbooks:** When ResolverProofStale fires, block new GAR entries, rotate cache-version expectations, and attach the latest enforcement receipt set to the rollback bundle.\n"
    )
}

fn render_security_baseline(pop_label: &str) -> String {
    format!(
        "# Security Baseline & PQ Readiness ({pop_label}, SN15-M0-12)\n\
\n\
- **TLS/ECH:** Store cert/ECH material in HSM-backed vault entries `vault://soranet/{pop_label}/tls` and `.../ech`; rotate quarterly with SRCv2 dual-sig (Ed25519 + ML-DSA) and record attestation hashes in `ops_summary.json`.\n\
- **Sandboxing:** Run gateway processes under dedicated cgroups with eBPF seccomp profiles; isolate WAF and verifier helpers in separate namespaces with read-only roots.\n\
- **SBOM + scanning:** Generate SPDX SBOMs during CI for edge + verifier images and run nightly vulnerability scans; attach latest scan summary to the promotion checklist.\n\
- **Log retention:** Default log retention to 30d with opt-in extensions; Loki buckets enforce deletion after expiry to satisfy privacy guarantees.\n\
- **PQ guardrails:** Enable Kyber-capable TLS ciphers on the edge once SRCv2 certs land; keep anonymous SoraNet circuits preferred and record downgrade counts in dashboards.\n\
- **Incident handling:** Tabletop quarterly for cache poison + resolver stale-proof scenarios with clear rollback hooks and Alertmanager routing to GAR/legal on-call roles.\n"
    )
}

fn render_pq_checklist(pop_label: &str) -> Value {
    let canary1 = format!("canary1.{pop_label}.gw.sora.id");
    let canary2 = format!("canary2.{pop_label}.gw.sora.id");

    norito::json!({
        "pop": pop_label,
        "tls_keys": {
            "status": "pending",
            "notes": "Rotate to SRCv2 dual-sig certs and publish attestation hash."
        },
        "sdr_receipts": {
            "status": "ready",
            "path": "/var/lib/soranet/sdr/receipts",
            "notes": "Ensure receipts are mirrored to ClickHouse for audit joins."
        },
        "cache_binding": {
            "status": "ready",
            "expect_header": "sora-cache-version",
            "notes": "Gateway rejects unknown cache versions; align with honey probes."
        },
        "canary_hosts": {
            "status": "ready",
            "hosts": [
                canary1,
                canary2
            ],
            "notes": "Use canaries for PQ cipher rollout before flipping defaults."
        }
    })
}

fn render_chaos_plan(
    pop_label: &str,
    scenarios: &[ChaosScenario],
    schedule: &[ScheduleEntry],
) -> String {
    let mut out = String::new();
    out.push_str("# SNNet-15F1 Chaos & GameDay Plan\n\n");
    out.push_str(&format!(
        "- **Pop:** `{pop_label}`\n- **Harness:** `cargo xtask soranet-gateway-chaos --pop {pop_label} --config chaos_scenarios.json --out artifacts/soranet/gateway_chaos`\n- **Artefacts:** `chaos_runbook.md`, `gameday_schedule.json`, `chaos_plan.md`, `chaos_metrics.json`, `chaos_injector.sh`, harness outputs under `artifacts/soranet/gateway_chaos/`.\n\n"
    ));
    out.push_str(
        "- **Evidence:** Keep Grafana screenshots (cache hit, resolver proof, latency), Alertmanager exports, and the JSON/Markdown outputs from the harness/injector.\n\
- **Logging:** Append every drill to `ops/drill-log.md` using `scripts/telemetry/log_sorafs_drill.sh --scenario <id> --status <pass|fail|follow-up>`.\n\
- **Rollback:** Every inject step must include the rollback command in the injector notes; operators should practice it even when running in simulate mode.\n\n",
    );
    out.push_str("## Quarterly schedule\n");
    for entry in schedule {
        out.push_str(&format!(
            "- {} → {}\n",
            entry.quarter,
            entry.scenarios.join(", ")
        ));
    }
    out.push_str("\n## Scenarios\n\n");
    for scenario in scenarios {
        out.push_str(&format!("### {} — {}\n\n", scenario.title, scenario.id));
        out.push_str(&format!(
            "Alert: `{}` · Alert budget: {}s · Recovery budget: {}s · Backlog cap: {}%\n\n",
            scenario.alert,
            scenario.benchmarks.alert_budget_seconds,
            scenario.benchmarks.recovery_budget_seconds,
            scenario.benchmarks.max_backlog_percent
        ));
        out.push_str(&format!("{}\n\n", scenario.description));
        out.push_str("**Inject steps:**\n");
        for step in &scenario.inject {
            out.push_str(&format!(
                "- {} ({}s) — `{}`\n",
                step.name,
                step.timeout_seconds,
                step.command.join(" ")
            ));
        }
        out.push('\n');
        out.push_str("**Verification:**\n");
        for step in &scenario.verify {
            out.push_str(&format!(
                "- {} ({}s) — `{}`\n",
                step.name,
                step.timeout_seconds,
                step.command.join(" ")
            ));
        }
        out.push('\n');
        out.push_str("**Remediation:**\n");
        for item in &scenario.remediate {
            out.push_str(&format!(
                "- {} ({}s) — `{}`\n",
                item.name,
                item.timeout_seconds,
                item.command.join(" ")
            ));
        }
        out.push_str("\n---\n\n");
    }
    out
}

fn render_chaos_metrics(pop_label: &str) -> Value {
    let mut bgp = json::Map::new();
    bgp.insert("id".into(), norito::json!("bgp_flap_recovery"));
    bgp.insert(
        "expr".into(),
        norito::json!(format!(
            "changes(soranet_bgp_session_up{{pop=\"{pop}\"}}[10m]) <= 2",
            pop = pop_label
        )),
    );
    bgp.insert(
        "goal".into(),
        norito::json!("BgpSessionFlap clears after rollback; convergence within 10 minutes."),
    );

    let mut resolver = json::Map::new();
    resolver.insert("id".into(), norito::json!("resolver_proof_age"));
    resolver.insert(
        "expr".into(),
        norito::json!(format!(
            "max_over_time(soradns_rad_proof_age_seconds{{pop=\"{pop}\"}}[15m]) < 900",
            pop = pop_label
        )),
    );
    resolver.insert(
        "goal".into(),
        norito::json!("Proofs refresh within 15 minutes after brownout recovery."),
    );

    let mut cache = json::Map::new();
    cache.insert("id".into(), norito::json!("cache_hit_recovery"));
    cache.insert(
        "expr".into(),
        norito::json!(format!(
            "(sum(rate(sorafs_gateway_cache_hits_total{{pop=\"{pop}\"}}[5m])) / sum(rate(sorafs_gateway_cache_requests_total{{pop=\"{pop}\"}}[5m]))) >= 0.92",
            pop = pop_label
        )),
    );
    cache.insert(
        "goal".into(),
        norito::json!("Cache hit rate returns to >=92% after verifier restart."),
    );

    let mut tail = json::Map::new();
    tail.insert("id".into(), norito::json!("tail_sampling_clear"));
    tail.insert(
        "expr".into(),
        norito::json!(format!(
            "sum(rate(otel_tail_sample_dropped_spans{{pop=\"{pop}\"}}[5m])) == 0",
            pop = pop_label
        )),
    );
    tail.insert(
        "goal".into(),
        norito::json!("Tail sampling drops back to 0 once chaos run ends."),
    );

    let checks = vec![
        Value::Object(bgp),
        Value::Object(resolver),
        Value::Object(cache),
        Value::Object(tail),
    ];
    let mut root = json::Map::new();
    root.insert("pop".into(), norito::json!(pop_label));
    root.insert("checks".into(), Value::Array(checks));
    root.insert(
        "instructions".into(),
        norito::json!("Run the expressions against Prometheus or store a range query JSON next to the injector outputs for audit. Attach matching rows to chaos_report.{json,md}."),
    );
    Value::Object(root)
}

fn render_chaos_injector(pop_label: &str, scenarios: &[ChaosScenario]) -> String {
    let mut supported = String::new();
    let mut supported_list = String::new();
    for scenario in scenarios {
        supported.push_str("        ");
        supported.push_str(&scenario.id);
        supported.push_str(")\n            SELECTED=\"");
        supported.push_str(&scenario.id);
        supported.push_str("\"\n            ;;\n");
        supported_list.push_str("  - ");
        supported_list.push_str(&scenario.id);
        supported_list.push('\n');
    }

    format!(
        "#!/usr/bin/env bash\n\
set -euo pipefail\n\
\n\
SCENARIO=\"\"\n\
OUTPUT_DIR=\"\"\n\
EXECUTE=0\n\
NOTE=\"\"\n\
\n\
usage() {{\n\
    cat <<'USAGE'\n\
Usage: chaos_injector.sh --scenario <id> [--output <dir>] [--execute] [--note <text>]\n\
\n\
Records a SNNet-15F1 chaos run using the xtask harness. Default mode is dry-run; pass --execute to run inject steps.\n\
Supported scenarios:\n\
{supported_list}USAGE\n\
}}\n\
\n\
while [[ $# -gt 0 ]]; do\n\
    case \"$1\" in\n\
        --scenario)\n\
            SCENARIO=$2\n\
            shift 2\n\
            ;;\n\
        --output)\n\
            OUTPUT_DIR=$2\n\
            shift 2\n\
            ;;\n\
        --execute)\n\
            EXECUTE=1\n\
            shift 1\n\
            ;;\n\
        --note)\n\
            NOTE=$2\n\
            shift 2\n\
            ;;\n\
        --help|-h)\n\
            usage\n\
            exit 0\n\
            ;;\n\
        *)\n\
            echo \"Unknown argument: $1\" >&2\n\
            usage\n\
            exit 1\n\
            ;;\n\
    esac\n\
done\n\
\n\
if [[ -z \"${{SCENARIO}}\" ]]; then\n\
    echo \"--scenario is required\" >&2\n\
    usage\n\
    exit 1\n\
fi\n\
\n\
case \"${{SCENARIO}}\" in\n\
{supported}        *)\n\
            echo \"Unsupported scenario '${{SCENARIO}}'\" >&2\n\
            echo \"Supported scenarios:\"\n\
{supported_list}            exit 1\n\
            ;;\n\
    esac\n\
\n\
SCRIPT_DIR=\"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")\" && pwd)\"\n\
CONFIG_PATH=\"${{SCRIPT_DIR}}/chaos_scenarios.json\"\n\
if [[ -z \"${{OUTPUT_DIR}}\" ]]; then\n\
    OUTPUT_DIR=\"artifacts/soranet/gateway_chaos\"\n\
fi\n\
\n\
mkdir -p \"${{OUTPUT_DIR}}\"\n\
CMD=(cargo xtask soranet-gateway-chaos --pop {pop} --config \"${{CONFIG_PATH}}\" --out \"${{OUTPUT_DIR}}\" --scenario \"${{SCENARIO}}\")\n\
if [[ ${{EXECUTE}} -eq 1 ]]; then\n\
    CMD+=(--execute)\n\
fi\n\
if [[ -n \"${{NOTE}}\" ]]; then\n\
    CMD+=(--note \"${{NOTE}}\")\n\
fi\n\
\n\
echo \"Running ${{SCENARIO}} (execute=${{EXECUTE}}) -> ${{OUTPUT_DIR}}\"\n\
\"${{CMD[@]}}\"\n\
echo \"Finished. Inspect chaos_report.json/md under ${{OUTPUT_DIR}}/run_* for evidence.\"\n",
        supported = supported,
        supported_list = supported_list,
        pop = pop_label,
    )
}

fn build_gameday_rotation(
    pops: &[String],
    outcomes: &[GatewayOpsOutcome],
) -> Result<Vec<GamedayRotationEntry>> {
    let mut rotation = Vec::new();
    for (idx, outcome) in outcomes.iter().enumerate() {
        let pop = pops
            .get(idx)
            .map(|p| sanitize_label(p))
            .unwrap_or_else(|| "unknown-pop".to_string());
        let schedule_bytes = fs::read(&outcome.gameday_schedule_path).wrap_err_with(|| {
            format!(
                "failed to read GameDay schedule {}",
                outcome.gameday_schedule_path.display()
            )
        })?;
        let schedule: Vec<ScheduleEntry> =
            json::from_slice(&schedule_bytes).wrap_err("invalid GameDay schedule JSON")?;
        for entry in schedule {
            rotation.push(GamedayRotationEntry {
                pop: pop.clone(),
                quarter: entry.quarter,
                scenarios: entry.scenarios,
                owner: entry.owner,
            });
        }
    }
    rotation.sort_by(|a, b| a.quarter.cmp(&b.quarter).then_with(|| a.pop.cmp(&b.pop)));
    Ok(rotation)
}

fn render_gameday_rotation_markdown(entries: &[GamedayRotationEntry]) -> String {
    let mut out = String::new();
    out.push_str("# Federated GameDay Rotation\n\n");
    out.push_str(
        "Rotates SNNet-15F chaos scenarios across active PoPs. Generated by `cargo xtask soranet-gateway-ops-m1`.\n\n",
    );
    for entry in entries {
        out.push_str(&format!(
            "- {} — `{}` → {}\n",
            entry.quarter,
            entry.pop,
            entry.scenarios.join(", ")
        ));
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

fn summarize_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fs};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn writes_chaos_assets() {
        let dir = tempdir().expect("tempdir");
        let opts = GatewayOpsOptions {
            output_dir: dir.path().to_path_buf(),
            pop: "Pop-Test".to_string(),
        };
        let outcome = write_gateway_ops_pack(opts).expect("write pack");

        let plan = fs::read_to_string(&outcome.chaos_plan_path).expect("chaos plan");
        assert!(
            plan.contains("prefix-withdrawal"),
            "chaos plan should mention prefix withdrawal"
        );

        let scenarios = fs::read(&outcome.chaos_scenarios_path).expect("chaos scenarios json");
        let scenario_pack: ScenarioPack =
            json::from_slice(&scenarios).expect("scenario pack parses");
        assert!(
            scenario_pack
                .scenarios
                .iter()
                .any(|scenario| scenario.id == "trustless-verifier-failure"),
            "scenario JSON should include verifier failure id"
        );

        let runbook = fs::read_to_string(&outcome.chaos_runbook_path).expect("chaos runbook");
        assert!(
            runbook.contains("Gateway Chaos Runbook"),
            "runbook should describe chaos harness usage"
        );

        let schedule_bytes =
            fs::read(&outcome.gameday_schedule_path).expect("gameday schedule exists");
        let quarterly: Vec<ScheduleEntry> =
            json::from_slice(&schedule_bytes).expect("schedule parses");
        assert_eq!(
            quarterly.len(),
            4,
            "schedule should include four quarterly entries"
        );

        let metrics = fs::read_to_string(&outcome.chaos_metrics_path).expect("chaos metrics json");
        assert!(
            metrics.contains("cache_hits_total"),
            "metrics JSON should include cache hit expression"
        );

        let injector =
            fs::read_to_string(&outcome.chaos_injector_path).expect("chaos injector script");
        assert!(
            injector.contains("soranet-gateway-chaos"),
            "injector script should wrap the chaos harness"
        );
    }

    #[test]
    fn writes_federated_pack() {
        let dir = tempdir().expect("tempdir");
        let opts = GatewayOpsMultiOptions {
            output_dir: dir.path().to_path_buf(),
            pops: vec!["sjc-01".to_string(), "iad-01".to_string()],
        };
        let outcome = write_gateway_ops_federated_pack(opts).expect("write federated pack");
        assert_eq!(outcome.per_pop.len(), 2, "should build per-pop packs");

        let federated =
            fs::read_to_string(&outcome.federated_otel_config_path).expect("federated otel config");
        assert!(
            federated.contains("kafka/sjc-01") && federated.contains("kafka/iad-01"),
            "federated config should include kafka receivers for each pop"
        );

        let rotation_bytes =
            fs::read(&outcome.gameday_rotation_path).expect("rotation schedule exists");
        let rotation: Vec<GamedayRotationEntry> =
            json::from_slice(&rotation_bytes).expect("rotation parses");
        let seen_pops: HashSet<_> = rotation.iter().map(|entry| entry.pop.as_str()).collect();
        assert_eq!(seen_pops.len(), 2, "rotation should cover both pops");

        let summary = fs::read_to_string(&outcome.summary_path).expect("federated summary exists");
        assert!(
            summary.contains("gameday_rotation.json"),
            "summary should mention rotation artefact"
        );
    }
}
