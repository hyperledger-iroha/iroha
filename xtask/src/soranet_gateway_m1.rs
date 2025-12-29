//! SNNet-15M1 alpha bundle generator for the SoraGlobal Gateway CDN.
//! Orchestrates PoP provisioning bundles, gateway/resolver baselines,
//! federated ops packs, and billing dry-runs into a single evidence root.

use std::{
    fs,
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr, eyre};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};

use crate::{soranet_gateway, soranet_gateway_billing, soranet_gateway_ops, soranet_pop};

#[derive(Debug)]
pub struct GatewayM1Options {
    pub config_path: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, JsonDeserialize)]
struct GatewayM1Config {
    #[norito(default)]
    image_tag: Option<String>,
    pops: Vec<GatewayM1Pop>,
    billing: GatewayM1Billing,
}

#[derive(Debug, JsonDeserialize)]
struct GatewayM1Pop {
    name: String,
    #[norito(default)]
    edge_name: Option<String>,
    descriptor: PathBuf,
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
    image_tag: Option<String>,
}

#[derive(Debug, JsonDeserialize)]
struct GatewayM1Billing {
    usage: PathBuf,
    #[norito(default)]
    catalog: Option<PathBuf>,
    #[norito(default)]
    guardrails: Option<PathBuf>,
    payer: String,
    treasury: String,
    asset: String,
    #[norito(default)]
    allow_hard_cap: Option<bool>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct GatewayM1PopSummary {
    name: String,
    image_tag: String,
    bundle_manifest: String,
    gateway_summary: String,
    ops_summary: String,
}

#[derive(Debug, JsonSerialize)]
struct GatewayM1FederatedSummary {
    summary: String,
    otel_config: String,
    gameday_rotation_json: String,
    gameday_rotation_markdown: String,
}

#[derive(Debug, JsonSerialize)]
struct GatewayM1BillingSummary {
    invoice: String,
    ledger_projection: String,
    totals_micros: u64,
}

#[derive(Debug, JsonSerialize)]
struct GatewayM1Summary {
    pops: Vec<GatewayM1PopSummary>,
    federated_ops: GatewayM1FederatedSummary,
    billing: GatewayM1BillingSummary,
}

#[derive(Debug)]
pub struct GatewayM1Outcome {
    pub summary_path: PathBuf,
}

/// Build the M1 alpha evidence bundle using a Norito JSON config.
pub fn run_gateway_m1(options: GatewayM1Options) -> Result<GatewayM1Outcome> {
    let config_bytes = fs::read(&options.config_path).wrap_err_with(|| {
        format!(
            "failed to read gateway M1 config {}",
            options.config_path.display()
        )
    })?;
    let config: GatewayM1Config = json::from_slice(&config_bytes).wrap_err_with(|| {
        format!(
            "failed to parse gateway M1 config {} as Norito JSON",
            options.config_path.display()
        )
    })?;
    if config.pops.is_empty() {
        return Err(eyre!("gateway M1 config must include at least one pop"));
    }
    let config_dir = options
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create gateway M1 output dir {}",
            options.output_dir.display()
        )
    })?;

    let default_image_tag = config
        .image_tag
        .clone()
        .unwrap_or_else(|| "soranet-gateway-m1-alpha".to_string());
    let pops_root = options.output_dir.join("pops");
    let mut pop_summaries = Vec::new();
    let mut pop_names = Vec::new();

    for pop in &config.pops {
        let pop_label = sanitize_label(&pop.name);
        let pop_dir = pops_root.join(&pop_label);
        let bundle_dir = pop_dir.join("bundle");
        let gateway_dir = pop_dir.join("gateway");
        let ops_dir = pop_dir.join("ops");

        let descriptor = resolve_path(config_dir, &pop.descriptor);
        let roa_bundle = pop
            .roa_bundle
            .as_ref()
            .map(|path| resolve_path(config_dir, path));
        let image_tag = pop
            .image_tag
            .clone()
            .unwrap_or_else(|| default_image_tag.clone());
        let skip_edns = pop.skip_edns.unwrap_or(false);
        let skip_ds = pop.skip_ds.unwrap_or(false);
        let edns_resolver = pop.edns_resolver.clone();
        let edns_tool = pop.edns_tool.clone();

        pop_names.push(pop.name.clone());

        let bundle_outcome = soranet_pop::build_pop_bundle(soranet_pop::PopBundleOptions {
            descriptor,
            roa_bundle,
            output_dir: bundle_dir,
            edns_resolver,
            edns_tool,
            skip_edns,
            skip_ds,
            image_tag: image_tag.clone(),
        })?;

        let gateway_outcome =
            soranet_gateway::write_gateway_m0_pack(soranet_gateway::GatewayM0Options {
                output_dir: gateway_dir,
                edge_name: pop
                    .edge_name
                    .clone()
                    .unwrap_or_else(|| format!("{}-edge", pop_label)),
            })?;

        let ops_outcome =
            soranet_gateway_ops::write_gateway_ops_pack(soranet_gateway_ops::GatewayOpsOptions {
                output_dir: ops_dir,
                pop: pop.name.clone(),
            })?;

        pop_summaries.push(GatewayM1PopSummary {
            name: pop.name.clone(),
            image_tag,
            bundle_manifest: summarize_path(&bundle_outcome.manifest_path, &options.output_dir),
            gateway_summary: summarize_path(&gateway_outcome.summary_path, &options.output_dir),
            ops_summary: summarize_path(&ops_outcome.summary_path, &options.output_dir),
        });
    }

    let federated_outcome = soranet_gateway_ops::write_gateway_ops_federated_pack(
        soranet_gateway_ops::GatewayOpsMultiOptions {
            output_dir: options.output_dir.join("ops_federated"),
            pops: pop_names.clone(),
        },
    )?;

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
            allow_hard_cap: config.billing.allow_hard_cap.unwrap_or(false),
        })?;

    let summary = GatewayM1Summary {
        pops: pop_summaries.clone(),
        federated_ops: GatewayM1FederatedSummary {
            summary: summarize_path(&federated_outcome.summary_path, &options.output_dir),
            otel_config: summarize_path(
                &federated_outcome.federated_otel_config_path,
                &options.output_dir,
            ),
            gameday_rotation_json: summarize_path(
                &federated_outcome.gameday_rotation_path,
                &options.output_dir,
            ),
            gameday_rotation_markdown: summarize_path(
                &federated_outcome.gameday_rotation_markdown_path,
                &options.output_dir,
            ),
        },
        billing: GatewayM1BillingSummary {
            invoice: summarize_path(&billing_outcome.invoice_path, &options.output_dir),
            ledger_projection: summarize_path(&billing_outcome.ledger_path, &options.output_dir),
            totals_micros: billing_outcome.total_micros,
        },
    };

    let summary_path = options.output_dir.join("gateway_m1_summary.json");
    let summary_file = fs::File::create(&summary_path)
        .wrap_err_with(|| format!("create {}", summary_path.display()))?;
    json::to_writer_pretty(summary_file, &summary)
        .wrap_err_with(|| format!("write {}", summary_path.display()))?;

    Ok(GatewayM1Outcome { summary_path })
}

fn resolve_path(base: &Path, candidate: &Path) -> PathBuf {
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.join(candidate)
    }
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
