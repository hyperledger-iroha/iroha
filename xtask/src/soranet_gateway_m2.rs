//! SNNet-15M2 beta bundle generator for the SoraGlobal Gateway CDN.
//! Extends the M1 alpha pack with DoQ/ODoH preview configs, trustless CAR
//! verifier wiring, PQ readiness evidence, GAR compliance rollup, and
//! hardening baselines ahead of GA.

use std::{
    fs,
    path::{Path, PathBuf},
};

use blake3::Hasher as Blake3;
use eyre::{Result, WrapErr, eyre};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};

use crate::{
    gar, soranet_gateway, soranet_gateway_billing, soranet_gateway_hardening, soranet_gateway_ops,
    soranet_gateway_pq, soranet_pop,
};

/// Command options for the M2 beta generator.
#[derive(Debug)]
pub struct GatewayM2Options {
    /// Norito JSON config.
    pub config_path: PathBuf,
    /// Output directory.
    pub output_dir: PathBuf,
}

#[derive(Debug, JsonDeserialize)]
struct GatewayM2Config {
    pops: Vec<GatewayM2Pop>,
    billing: GatewayM2Billing,
    #[norito(default)]
    compliance: Option<GatewayM2Compliance>,
    #[norito(default)]
    hardening: Option<GatewayM2Hardening>,
}

#[derive(Debug, JsonDeserialize)]
struct GatewayM2Pop {
    name: String,
    descriptor: PathBuf,
    #[norito(default)]
    edge_name: Option<String>,
    #[norito(default)]
    image_tag: Option<String>,
    #[norito(default)]
    roa_bundle: Option<PathBuf>,
    #[norito(default)]
    edns_resolver: Option<String>,
    #[norito(default)]
    edns_tool: Option<String>,
    #[norito(default)]
    skip_edns: Option<bool>,
    #[norito(default)]
    skip_ds: Option<bool>,
    #[norito(default)]
    trustless_config: Option<PathBuf>,
    #[norito(default)]
    pq_bundle: Option<PathBuf>,
    #[norito(default)]
    tls_bundle_dir: Option<PathBuf>,
    #[norito(default)]
    doq_listen: Option<Vec<String>>,
    #[norito(default)]
    odoh_relay: Option<String>,
}

#[derive(Debug, JsonDeserialize)]
struct GatewayM2Billing {
    usage: PathBuf,
    payer: String,
    treasury: String,
    asset: String,
    #[norito(default)]
    catalog: Option<PathBuf>,
    #[norito(default)]
    guardrails: Option<PathBuf>,
}

#[derive(Debug, JsonDeserialize)]
struct GatewayM2Compliance {
    receipts_dir: PathBuf,
    #[norito(default)]
    acks_dir: Option<PathBuf>,
}

#[derive(Debug, JsonDeserialize)]
struct GatewayM2Hardening {
    #[norito(default)]
    sbom: Option<PathBuf>,
    #[norito(default)]
    vuln_report: Option<PathBuf>,
    #[norito(default)]
    hsm_policy: Option<PathBuf>,
    #[norito(default)]
    sandbox_profile: Option<PathBuf>,
    #[norito(default = "default_data_retention")]
    data_retention_days: u32,
    #[norito(default = "default_log_retention")]
    log_retention_days: u32,
}

#[derive(Debug, Clone, JsonSerialize)]
struct GatewayM2PopSummary {
    name: String,
    image_tag: String,
    pop_bundle: String,
    gateway_summary: String,
    beta_edge_config: String,
    ops_summary: String,
    trustless_config: String,
    #[norito(default)]
    pq_summary: Option<String>,
    #[norito(default)]
    hardening_summary: Option<String>,
}

#[derive(Debug, JsonSerialize)]
struct GatewayM2BillingSummary {
    invoice: String,
    ledger_projection: String,
    totals_micros: u64,
}

#[derive(Debug, JsonSerialize)]
struct GatewayM2Summary {
    pops: Vec<GatewayM2PopSummary>,
    billing: GatewayM2BillingSummary,
    #[norito(default)]
    compliance_summary: Option<String>,
}

/// Outcome for the M2 beta generator.
#[derive(Debug)]
pub struct GatewayM2Outcome {
    /// Path to the summary JSON file.
    pub summary_path: PathBuf,
}

/// Build the M2 beta bundle given a Norito JSON config.
pub fn run_gateway_m2(options: GatewayM2Options) -> Result<GatewayM2Outcome> {
    let config_bytes = fs::read(&options.config_path).wrap_err_with(|| {
        format!(
            "failed to read gateway M2 config {}",
            options.config_path.display()
        )
    })?;
    let config: GatewayM2Config = json::from_slice(&config_bytes).wrap_err_with(|| {
        format!(
            "failed to parse gateway M2 config {} as Norito JSON",
            options.config_path.display()
        )
    })?;
    if config.pops.is_empty() {
        return Err(eyre!("gateway M2 config must include at least one pop"));
    }
    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create gateway M2 output dir {}",
            options.output_dir.display()
        )
    })?;

    let config_dir = options
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let mut pop_summaries = Vec::new();

    let pops_root = options.output_dir.join("pops");
    for pop in &config.pops {
        let pop_label = sanitize_label(&pop.name);
        let pop_dir = pops_root.join(&pop_label);
        let bundle_dir = pop_dir.join("bundle");
        let gateway_dir = pop_dir.join("gateway");
        let ops_dir = pop_dir.join("ops");
        let beta_dir = pop_dir.join("beta");

        let descriptor = resolve_path(config_dir, &pop.descriptor);
        let roa_bundle = pop
            .roa_bundle
            .as_ref()
            .map(|path| resolve_path(config_dir, path));

        let m0_outcome =
            soranet_gateway::write_gateway_m0_pack(soranet_gateway::GatewayM0Options {
                output_dir: gateway_dir.clone(),
                edge_name: pop
                    .edge_name
                    .clone()
                    .unwrap_or_else(|| format!("{}-edge", pop_label)),
            })?;

        let trustless_config = pop
            .trustless_config
            .as_ref()
            .map(|path| resolve_path(config_dir, path))
            .unwrap_or(m0_outcome.trustless_verifier_path.clone());

        fs::create_dir_all(&beta_dir)
            .wrap_err_with(|| format!("failed to create beta dir `{}`", beta_dir.display()))?;
        let beta_edge_path = beta_dir.join("gateway_edge_beta.yaml");
        fs::write(
            &beta_edge_path,
            render_edge_beta_config(
                pop.edge_name
                    .clone()
                    .unwrap_or_else(|| format!("{}-edge", pop_label)),
                &trustless_config,
                pop.doq_listen.as_deref().unwrap_or(&[]),
                pop.odoh_relay.as_deref(),
            ),
        )
        .wrap_err_with(|| format!("failed to write {}", beta_edge_path.display()))?;

        let bundle_outcome = soranet_pop::build_pop_bundle(soranet_pop::PopBundleOptions {
            descriptor,
            roa_bundle,
            output_dir: bundle_dir,
            edns_resolver: pop.edns_resolver.clone(),
            edns_tool: pop.edns_tool.clone(),
            skip_edns: pop.skip_edns.unwrap_or(false),
            skip_ds: pop.skip_ds.unwrap_or(false),
            image_tag: pop
                .image_tag
                .clone()
                .unwrap_or_else(|| "soranet-gateway-m2-beta".to_string()),
        })?;

        let ops_outcome =
            soranet_gateway_ops::write_gateway_ops_pack(soranet_gateway_ops::GatewayOpsOptions {
                output_dir: ops_dir,
                pop: pop.name.clone(),
            })?;

        let mut pq_summary = None;
        if let (Some(pq_bundle), Some(tls_dir)) = (&pop.pq_bundle, &pop.tls_bundle_dir) {
            let pq_outcome =
                soranet_gateway_pq::run_gateway_pq_readiness(soranet_gateway_pq::GatewayPqOptions {
                    output_dir: beta_dir.join("pq"),
                    pop: pop.name.clone(),
                    srcv2_bundle: resolve_path(config_dir, pq_bundle),
                    tls_bundle_dir: resolve_path(config_dir, tls_dir),
                    trustless_config: trustless_config.clone(),
                    canary_hosts: Vec::new(),
                    validation_phase: iroha_crypto::soranet::certificate::CertificateValidationPhase::Phase3RequireDual,
                })?;
            pq_summary = Some(summarize_path(
                &pq_outcome.summary_json,
                &options.output_dir,
            ));
        }

        let mut hardening_summary = None;
        if let Some(harden_cfg) = &config.hardening {
            let outcome = soranet_gateway_hardening::run_gateway_hardening(
                soranet_gateway_hardening::GatewayHardeningOptions {
                    output_dir: beta_dir.join("hardening"),
                    sbom: harden_cfg
                        .sbom
                        .as_ref()
                        .map(|path| resolve_path(config_dir, path)),
                    vuln_report: harden_cfg
                        .vuln_report
                        .as_ref()
                        .map(|path| resolve_path(config_dir, path)),
                    hsm_policy: harden_cfg
                        .hsm_policy
                        .as_ref()
                        .map(|path| resolve_path(config_dir, path)),
                    sandbox_profile: harden_cfg
                        .sandbox_profile
                        .as_ref()
                        .map(|path| resolve_path(config_dir, path)),
                    data_retention_days: harden_cfg.data_retention_days,
                    log_retention_days: harden_cfg.log_retention_days,
                },
            )?;
            hardening_summary = Some(summarize_path(&outcome.summary_json, &options.output_dir));
        }

        pop_summaries.push(GatewayM2PopSummary {
            name: pop.name.clone(),
            image_tag: pop
                .image_tag
                .clone()
                .unwrap_or_else(|| "soranet-gateway-m2-beta".to_string()),
            pop_bundle: summarize_path(&bundle_outcome.manifest_path, &options.output_dir),
            gateway_summary: summarize_path(&m0_outcome.summary_path, &options.output_dir),
            beta_edge_config: summarize_path(&beta_edge_path, &options.output_dir),
            ops_summary: summarize_path(&ops_outcome.summary_path, &options.output_dir),
            trustless_config: summarize_path(&trustless_config, &options.output_dir),
            pq_summary,
            hardening_summary,
        });
    }

    let billing_dir = options.output_dir.join("billing");
    let default_catalog = PathBuf::from("../gateway_m0/meter_catalog.json");
    let catalog_path = resolve_path(
        config_dir,
        config.billing.catalog.as_ref().unwrap_or(&default_catalog),
    );
    let guardrails_path = config
        .billing
        .guardrails
        .as_ref()
        .map(|path| resolve_path(config_dir, path));
    let billing_outcome =
        soranet_gateway_billing::run_billing(soranet_gateway_billing::BillingOptions {
            usage_path: resolve_path(config_dir, &config.billing.usage),
            catalog_path,
            guardrails_path,
            output_dir: billing_dir,
            payer: config.billing.payer.clone(),
            treasury: config.billing.treasury.clone(),
            asset_definition: config.billing.asset.clone(),
            allow_hard_cap: true,
        })?;

    let mut compliance_summary = None;
    if let Some(compliance) = &config.compliance {
        let out_path = options.output_dir.join("compliance_summary.json");
        let summary = gar::export_receipts(gar::ExportOptions {
            receipts_dir: resolve_path(config_dir, &compliance.receipts_dir),
            ack_dir: compliance
                .acks_dir
                .as_ref()
                .map(|path| resolve_path(config_dir, path)),
            pop_label: None,
            markdown_out: Some(options.output_dir.join("compliance_summary.md")),
            now_unix: None,
        })?;
        let file = fs::File::create(&out_path)
            .wrap_err_with(|| format!("create {}", out_path.display()))?;
        json::to_writer_pretty(file, &summary)
            .wrap_err_with(|| format!("write {}", out_path.display()))?;
        compliance_summary = Some(summarize_path(&out_path, &options.output_dir));
    }

    let summary = GatewayM2Summary {
        pops: pop_summaries,
        billing: GatewayM2BillingSummary {
            invoice: summarize_path(&billing_outcome.invoice_path, &options.output_dir),
            ledger_projection: summarize_path(&billing_outcome.ledger_path, &options.output_dir),
            totals_micros: billing_outcome.total_micros,
        },
        compliance_summary,
    };

    let summary_path = options.output_dir.join("gateway_m2_summary.json");
    let summary_file = fs::File::create(&summary_path)
        .wrap_err_with(|| format!("create {}", summary_path.display()))?;
    json::to_writer_pretty(summary_file, &summary)
        .wrap_err_with(|| format!("write {}", summary_path.display()))?;

    Ok(GatewayM2Outcome { summary_path })
}

fn resolve_path(base: &Path, candidate: &Path) -> PathBuf {
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.join(candidate)
    }
}

fn render_edge_beta_config(
    edge_name: String,
    trustless_config: &Path,
    doq_listen: &[String],
    odoh_relay: Option<&str>,
) -> String {
    let label = sanitize_label(&edge_name);
    let mut out = String::new();
    let doq_section = if doq_listen.is_empty() {
        "  doq: []\n".to_string()
    } else {
        let rendered = doq_listen
            .iter()
            .map(|value| format!("    - {value}\n"))
            .collect::<String>();
        format!("  doq:\n{rendered}")
    };
    let odoh_section = odoh_relay
        .map(|relay| format!("odoh_relay: \"{relay}\"\n"))
        .unwrap_or_else(|| "odoh_relay: null\n".to_string());
    out.push_str(&format!(
        "# Gateway edge beta config for {edge_name}\n\
gateway:\n\
  name: {edge_name}\n\
  listen:\n\
    address: 0.0.0.0\n\
    port: 443\n\
    protocol: h3\n\
    tls:\n\
      cert_ref: \"vault://soranet/{label}/tls\"\n\
      ech_key_ref: \"vault://soranet/{label}/ech\"\n\
{doq_section}\
  privacy:\n\
    {odoh_section}\
  cache:\n\
    tiers:\n\
      - name: ram\n\
        max_entries: 12000\n\
      - name: nvme\n\
        size_gib: 512\n\
    serve_stale_seconds: 120\n\
  origins:\n\
    - name: sorafs-gateway\n\
      scheme: sorafs\n\
      trustless_verifier: \"{trustless}\"\n\
      rate_limits:\n\
        burst_rps: 600\n\
        sustained_rps: 300\n\
      waf_policy: \"../gateway_waf_policy.yaml\"\n\
      headers:\n\
        - name: sora-cache-version\n\
          required: true\n\
        - name: sora-denylist-version\n\
          required: true\n\
  observability:\n\
    prometheus_listener: 0.0.0.0:19092\n\
    trace_sample: 0.15\n\
  rollout_notes:\n\
    - \"M2 enablement: DoQ/ODoH preview with trustless verifier bound\"\n\
    - \"Include `gateway_m2_summary.json` in governance packets\"\n",
        trustless = trustless_config.display()
    ));
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
    match path.strip_prefix(root) {
        Ok(stripped) => stripped.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

fn default_data_retention() -> u32 {
    30
}

fn default_log_retention() -> u32 {
    30
}

/// Compute a BLAKE3 digest for the supplied file.
#[allow(dead_code)]
fn file_blake3_hex(path: &Path) -> Result<String> {
    let mut hasher = Blake3::new();
    let mut file = fs::File::open(path).wrap_err_with(|| format!("open {}", path.display()))?;
    std::io::copy(&mut file, &mut hasher).wrap_err_with(|| format!("hash {}", path.display()))?;
    Ok(hasher.finalize().to_hex().to_string())
}
