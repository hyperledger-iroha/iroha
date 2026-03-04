//! M0 gateway baseline pack for the SoraGlobal Gateway CDN (SNNet-15M0).
//! Generates H3 edge config, trustless verifier skeleton, and WAF/rate
//! policies so early PoPs can run consistent drills.

use std::{
    fs::{self, File},
    path::PathBuf,
};

use eyre::{Result, WrapErr};
use norito::{derive::JsonSerialize, json};

#[derive(Debug)]
pub struct GatewayM0Options {
    pub output_dir: PathBuf,
    pub edge_name: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct GatewayM0Outcome {
    pub summary_path: PathBuf,
    pub edge_config_path: PathBuf,
    pub trustless_verifier_path: PathBuf,
    pub waf_policy_path: PathBuf,
}

#[derive(Debug, JsonSerialize)]
struct GatewayM0Summary {
    edge_name: String,
    edge_config_path: String,
    trustless_verifier_path: String,
    waf_policy_path: String,
}

/// Write the gateway M0 pack to disk and return the summary paths.
pub fn write_gateway_m0_pack(options: GatewayM0Options) -> Result<GatewayM0Outcome> {
    let GatewayM0Options {
        output_dir,
        edge_name,
    } = options;
    fs::create_dir_all(&output_dir).wrap_err_with(|| {
        format!(
            "failed to create gateway M0 output directory `{}`",
            output_dir.display()
        )
    })?;

    let edge_config_path = output_dir.join("gateway_edge_h3.yaml");
    let trustless_verifier_path = output_dir.join("gateway_trustless_verifier.toml");
    let waf_policy_path = output_dir.join("gateway_waf_policy.yaml");
    let summary_path = output_dir.join("gateway_m0_summary.json");

    fs::write(&edge_config_path, render_edge_h3_config(&edge_name))
        .wrap_err_with(|| format!("failed to write edge config {}", edge_config_path.display()))?;
    fs::write(&trustless_verifier_path, render_trustless_verifier_config()).wrap_err_with(
        || {
            format!(
                "failed to write trustless verifier skeleton {}",
                trustless_verifier_path.display()
            )
        },
    )?;
    fs::write(&waf_policy_path, render_waf_policy_pack()).wrap_err_with(|| {
        format!(
            "failed to write WAF policy pack {}",
            waf_policy_path.display()
        )
    })?;

    let summary = GatewayM0Summary {
        edge_name: edge_name.clone(),
        edge_config_path: edge_config_path.display().to_string(),
        trustless_verifier_path: trustless_verifier_path.display().to_string(),
        waf_policy_path: waf_policy_path.display().to_string(),
    };
    let file = File::create(&summary_path).wrap_err_with(|| {
        format!(
            "failed to create gateway M0 summary {}",
            summary_path.display()
        )
    })?;
    json::to_writer_pretty(file, &summary).wrap_err_with(|| {
        format!(
            "failed to write gateway M0 summary {}",
            summary_path.display()
        )
    })?;

    Ok(GatewayM0Outcome {
        summary_path,
        edge_config_path,
        trustless_verifier_path,
        waf_policy_path,
    })
}

fn render_edge_h3_config(edge_name: &str) -> String {
    let label = sanitize_label(edge_name);
    format!(
        "# Gateway edge baseline for {edge}\n\
gateway:\n\
  name: {edge}\n\
  listen:\n\
    address: 0.0.0.0\n\
    port: 443\n\
    protocol: h3\n\
    tls:\n\
      cert_ref: \"vault://soranet/{label}/tls\"\n\
      ech_key_ref: \"vault://soranet/{label}/ech\"\n\
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
    - \"Update cert/ECH refs to match the staged vault entries before canaries.\"\n",
        edge = edge_name,
        label = label
    )
}

fn render_trustless_verifier_config() -> String {
    "# Trustless verifier skeleton for SN15-M0 (streaming Merkle/KZG checks)\n\
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
emit_metrics = true\n"
        .to_string()
}

fn render_waf_policy_pack() -> String {
    "# WAF + rate policy pack for SN15-M0\n\
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
    notes: \"False-positive harness kept in the pack so rollbacks are validated automatically\"\n\
"
    .to_string()
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
