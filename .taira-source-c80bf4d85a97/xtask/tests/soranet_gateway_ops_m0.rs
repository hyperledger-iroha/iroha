use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::tempdir;

#[test]
fn soranet_gateway_ops_m0_pack_is_deterministic() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("ops");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .args([
            "soranet-gateway-ops-m0",
            "--output-dir",
            out_dir.to_str().expect("utf8"),
            "--pop",
            "qa-pop_01",
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-gateway-ops-m0");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let otel_config =
        fs::read_to_string(out_dir.join("otel_collector.yaml")).expect("otel config exists");
    let expected_otel = "# OTEL collector profile for qa-pop-01 (SN15-M0-10)\n\
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
                pop: qa-pop-01\n\
                role: gateway\n\
        - job_name: resolver\n\
          scrape_interval: 15s\n\
          static_configs:\n\
            - targets: [\"127.0.0.1:9100\"]\n\
              labels:\n\
                pop: qa-pop-01\n\
                role: resolver\n\
  filelog:\n\
    include:\n\
      - /var/log/soranet/*.log\n\
    operators:\n\
      - type: add\n\
        field: attributes.pop\n\
        value: qa-pop-01\n\
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
    topic: otel-qa-pop-01\n\
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
      exporters: [loki, clickhouse]\n";
    assert_eq!(otel_config, expected_otel, "otel config drifted");

    let dashboard_bytes =
        fs::read(out_dir.join("grafana_dashboard.json")).expect("dashboard exists");
    let dashboard: Value = json::from_slice(&dashboard_bytes).expect("dashboard parses");
    let expected_dashboard = norito::json!({
        "title": "SoraGlobal Gateway M0 (qa-pop-01)",
        "tags": ["soranet", "m0", "qa-pop-01"],
        "time": { "from": "now-6h", "to": "now" },
        "panels": [
            {
                "id": 1,
                "type": "timeseries",
                "title": "Gateway latency p95",
                "targets": [{
                    "expr": "histogram_quantile(0.95, rate(sorafs_gateway_latency_ms_bucket{pop=\"qa-pop-01\"}[5m]))",
                    "legendFormat": "gateway latency p95",
                }],
                "description": "Target 350ms p95 with 500ms error budget ceiling."
            },
            {
                "id": 2,
                "type": "stat",
                "title": "Cache hit rate",
                "targets": [{
                    "expr": "sum(rate(sorafs_gateway_cache_hits_total{pop=\"qa-pop-01\"}[5m])) / sum(rate(sorafs_gateway_cache_requests_total{pop=\"qa-pop-01\"}[5m]))"
                }],
                "thresholds": {
                    "mode": "absolute",
                    "steps": [
                        { "color": "green", "value": 0.0 },
                        { "color": "orange", "value": 0.92 },
                        { "color": "red", "value": 0.89 }
                    ]
                },
                "description": "Hold >92% hit rate during drills; annotate dips with resolver/gateway incidents."
            },
            {
                "id": 3,
                "type": "stat",
                "title": "Error budget burn (hourly)",
                "targets": [{
                    "expr": "(1 - avg_over_time(sorafs_gateway_success_ratio{pop=\"qa-pop-01\"}[1h])) * 100"
                }],
                "description": "Keeps SN15-M0 burn rate visible for change approvals."
            },
            {
                "id": 4,
                "type": "timeseries",
                "title": "Resolver proof age",
                "targets": [{
                    "expr": "max_over_time(soradns_rad_proof_age_seconds{pop=\"qa-pop-01\"}[15m])"
                }],
                "description": "Alarm before RAD proofs age past 15m; ties to GAR policy enforcement."
            }
        ],
        "annotations": {
            "list": [{
                "name": "deploy",
                "enable": true,
                "expr": "changes(sorafs_gateway_deploy_revision{pop=\"qa-pop-01\"}[5m]) > 0"
            }]
        }
    });
    assert_eq!(dashboard, expected_dashboard, "dashboard drifted");

    let alerts = fs::read_to_string(out_dir.join("alert_rules.yml")).expect("alert rules exist");
    let expected_alerts = "# Prometheus alert rules for qa-pop-01 (SN15-M0-10/11)\n\
groups:\n\
- name: soranet-gateway-m0\n\
  rules:\n\
    - alert: GatewayScrapeMissing\n\
      expr: up{job=\"gateway\",pop=\"qa-pop-01\"} == 0\n\
      for: 2m\n\
      labels:\n\
        severity: critical\n\
        pop: qa-pop-01\n\
      annotations:\n\
        summary: \"Gateway scrape missing at qa-pop-01\"\n\
        description: \"Prometheus cannot reach the gateway metrics endpoint; check PoP health and firewall.\"\n\
    - alert: CacheHitRateDrop\n\
      expr: (sum(rate(sorafs_gateway_cache_hits_total{pop=\"qa-pop-01\"}[5m])) / sum(rate(sorafs_gateway_cache_requests_total{pop=\"qa-pop-01\"}[5m]))) < 0.89\n\
      for: 10m\n\
      labels:\n\
        severity: warning\n\
        pop: qa-pop-01\n\
      annotations:\n\
        summary: \"Cache hit rate below 89%\"\n\
        description: \"Investigate resolver health, origin latency, or cache eviction bursts before making the orchestrator default-on.\"\n\
    - alert: ResolverProofStale\n\
      expr: max_over_time(soradns_rad_proof_age_seconds{pop=\"qa-pop-01\"}[15m]) > 900\n\
      for: 1m\n\
      labels:\n\
        severity: critical\n\
        pop: qa-pop-01\n\
      annotations:\n\
        summary: \"RAD proof age exceeding 15 minutes\"\n\
        description: \"Refresh SoraDNS sync and verify GAR bindings; stale proofs block GAR enforcement checks.\"\n\
    - alert: BgpSessionFlap\n\
      expr: changes(soranet_bgp_session_up{pop=\"qa-pop-01\"}[10m]) > 2\n\
      for: 0m\n\
      labels:\n\
        severity: warning\n\
        pop: qa-pop-01\n\
      annotations:\n\
        summary: \"BGP session flapping\"\n\
        description: \"Coordinate with edge on-call to confirm drain/withdraw workflows and capture evidence for SN15-M0 drills.\"\n\
    - alert: TailSamplingBackpressure\n\
      expr: sum(rate(otel_tail_sample_dropped_spans{pop=\"qa-pop-01\"}[5m])) > 0\n\
      for: 5m\n\
      labels:\n\
        severity: warning\n\
        pop: qa-pop-01\n\
      annotations:\n\
        summary: \"Tail sampling is dropping spans\"\n\
        description: \"Tune sampling percentage or Kafka throughput; span loss invalidates change packets for observability GA.\"\n";
    assert_eq!(alerts, expected_alerts, "alert rules drifted");

    let pq_bytes =
        fs::read(out_dir.join("pq_readiness_checklist.json")).expect("pq checklist exists");
    let pq: Value = json::from_slice(&pq_bytes).expect("pq checklist parses");
    assert_eq!(pq["pop"], Value::from("qa-pop-01"));
    assert_eq!(
        pq["canary_hosts"],
        norito::json!({
            "status": "ready",
            "hosts": [
                "canary1.qa-pop-01.gw.sora.id",
                "canary2.qa-pop-01.gw.sora.id"
            ],
            "notes": "Use canaries for PQ cipher rollout before flipping defaults."
        })
    );

    let summary_bytes = fs::read(out_dir.join("ops_summary.json")).expect("summary exists");
    let summary: Value = json::from_slice(&summary_bytes).expect("summary parses");
    assert_eq!(summary["pop"], Value::from("qa-pop_01"));
    assert_eq!(
        summary["otel_config_path"],
        Value::from("otel_collector.yaml")
    );
    assert_eq!(
        summary["dashboard_path"],
        Value::from("grafana_dashboard.json")
    );
}
