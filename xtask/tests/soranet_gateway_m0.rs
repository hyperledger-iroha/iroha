use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::tempdir;

#[test]
fn soranet_gateway_m0_pack_is_deterministic() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("gateway");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .args([
            "soranet-gateway-m0",
            "--output-dir",
            out_dir.to_str().expect("utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-gateway-m0");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let edge_config =
        fs::read_to_string(out_dir.join("gateway_edge_h3.yaml")).expect("edge config exists");
    let expected_edge = "# Gateway edge baseline for soranet-edge-m0\n\
gateway:\n\
  name: soranet-edge-m0\n\
  listen:\n\
    address: 0.0.0.0\n\
    port: 443\n\
    protocol: h3\n\
    tls:\n\
      cert_ref: \"vault://soranet/soranet-edge-m0/tls\"\n\
      ech_key_ref: \"vault://soranet/soranet-edge-m0/ech\"\n\
  cache:\n\
    tiers:\n\
      - name: ram\n\
        max_entries: 12000\n\
      - name: nvme\n\
        size_gib: 256\n\
    serve_stale_seconds: 90\n\
  origins:\n\
    - name: sorafs-gateway\n\
      scheme: sorafs\n\
      trustless_verifier: \"../gateway_trustless_verifier.toml\"\n\
      rate_limits:\n\
        burst_rps: 400\n\
        sustained_rps: 200\n\
      waf_policy: \"../gateway_waf_policy.yaml\"\n\
  observability:\n\
    prometheus_listener: 0.0.0.0:19092\n\
    trace_sample: 0.10\n\
  rollout_notes:\n\
    - \"Use `gateway_m0_summary.json` in governance packets for evidence links.\"\n\
    - \"Update cert/ECH refs to match the staged vault entries before canaries.\"\n";
    assert_eq!(
        edge_config, expected_edge,
        "edge config drifted from baseline"
    );

    let verifier = fs::read_to_string(out_dir.join("gateway_trustless_verifier.toml"))
        .expect("trustless verifier exists");
    let expected_verifier = "# Trustless verifier skeleton for SN15-M0 (streaming Merkle/KZG checks)\n\
version = 1\n\
\n\
[merkle]\n\
chunk_window = 32\n\
max_parallel_streams = 8\n\
\n\
[kzg]\n\
trusted_setup = \"/var/lib/soranet/kzg/trusted_setup.params\"\n\
proof_cache = \"/var/lib/soranet/kzg/cache\"\n\
max_gap_ms = 250\n\
\n\
[sdr]\n\
receipt_dir = \"/var/lib/soranet/sdr/receipts\"\n\
max_lag_seconds = 4\n\
\n\
[pipeline]\n\
allow_hybrid_manifest = true\n\
reject_stale_cache_versions = true\n\
verify_cache_binding_header = true\n\
\n\
[logging]\n\
level = \"info\"\n\
emit_metrics = true\n";
    assert_eq!(
        verifier, expected_verifier,
        "trustless verifier skeleton drifted"
    );

    let waf_policy =
        fs::read_to_string(out_dir.join("gateway_waf_policy.yaml")).expect("waf policy exists");
    let expected_waf = "# WAF + rate policy pack for SN15-M0\n\
metadata:\n\
  name: soranet-waf-m0\n\
  version: 2\n\
  rollout:\n\
    shadow_mode_seconds: 300\n\
    rollback_playbook: docs/source/sorafs_gateway_deployment_handbook.md\n\
    fp_harness: ci/check_sorafs_gateway_conformance.sh\n\
rules:\n\
  - id: owasp-top10\n\
    stage: request\n\
    action: block\n\
    notes: \"Baseline OWASP checks for H3 ingress\"\n\
  - id: adaptive-rate-limit\n\
    stage: request\n\
    action: throttle\n\
    limit_rps: 200\n\
    burst: 50\n\
    interval_seconds: 10\n\
    soak_seconds: 120\n\
    notes: \"Rate cap with soft soak window for FP drills\"\n\
  - id: bot-heuristics\n\
    stage: request\n\
    action: challenge\n\
    signals:\n\
      - header: user-agent\n\
      - header: x-forwarded-for\n\
      - header: x-client-trace\n\
    notes: \"Challenge suspicious automation before origin dispatch\"\n\
  - id: json-parser-anomaly\n\
    stage: request\n\
    action: block\n\
    notes: \"Reject malformed JSON and oversize bodies before origin dispatch\"\n\
  - id: fp-harness\n\
    stage: response\n\
    action: allow\n\
    probes:\n\
      - path: /healthz\n\
        expect_status: 200\n\
      - path: /metrics\n\
        expect_status: 200\n\
    notes: \"False-positive harness kept in the pack so rollbacks are validated automatically\"\n";
    assert_eq!(waf_policy, expected_waf, "waf policy drifted");

    let summary_bytes = fs::read(out_dir.join("gateway_m0_summary.json")).expect("summary exists");
    let summary: Value = json::from_slice(&summary_bytes).expect("summary parses");
    assert_eq!(
        summary["edge_name"],
        Value::from("soranet-edge-m0"),
        "summary edge name mismatch"
    );
}
