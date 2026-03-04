//! SNNet-15M3 GA readiness bundle for the SoraGlobal Gateway CDN.
//! Consumes the M2 summary and emits GA artefacts (autoscale/worker
//! digests, SLA targets) for governance evidence packets.

use std::{
    fs,
    path::{Path, PathBuf},
};

use blake3::Hasher as Blake3;
use eyre::{Result, WrapErr};
use norito::json::{self, Map, Value};

/// Options for the GA bundle.
#[derive(Debug)]
pub struct GatewayM3Options {
    /// M2 summary JSON.
    pub m2_summary: PathBuf,
    /// Output directory.
    pub output_dir: PathBuf,
    /// Autoscale/route plan JSON.
    pub autoscale_plan: PathBuf,
    /// Worker pack archive (WASM/containers).
    pub worker_pack: PathBuf,
    /// Optional SLA target label.
    pub sla_target: Option<String>,
}

/// Output locations for the GA bundle.
#[derive(Debug)]
pub struct GatewayM3Outcome {
    /// JSON summary path.
    pub summary_json: PathBuf,
    /// Markdown summary path.
    pub summary_markdown: PathBuf,
}

/// Generate the GA artefact bundle.
pub fn run_gateway_m3(options: GatewayM3Options) -> Result<GatewayM3Outcome> {
    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create gateway M3 output dir `{}`",
            options.output_dir.display()
        )
    })?;

    let m2_summary_path = options.m2_summary.clone();
    let autoscale_digest = file_blake3_hex(&options.autoscale_plan)?;
    let worker_digest = file_blake3_hex(&options.worker_pack)?;

    let mut root = Map::new();
    root.insert(
        "m2_summary".into(),
        Value::String(relative(&m2_summary_path, &options.output_dir)),
    );
    root.insert(
        "autoscale_plan".into(),
        Value::String(relative(&options.autoscale_plan, &options.output_dir)),
    );
    root.insert(
        "autoscale_plan_blake3".into(),
        Value::String(autoscale_digest),
    );
    root.insert(
        "worker_pack".into(),
        Value::String(relative(&options.worker_pack, &options.output_dir)),
    );
    root.insert("worker_pack_blake3".into(), Value::String(worker_digest));
    if let Some(sla) = &options.sla_target {
        root.insert("sla_target".into(), Value::String(sla.clone()));
    }

    let summary_json = options.output_dir.join("gateway_m3_summary.json");
    let summary_markdown = options.output_dir.join("gateway_m3_summary.md");

    let json_payload = Value::Object(root.clone());
    let file = fs::File::create(&summary_json)
        .wrap_err_with(|| format!("create {}", summary_json.display()))?;
    json::to_writer_pretty(file, &json_payload)
        .wrap_err_with(|| format!("write {}", summary_json.display()))?;

    let markdown = render_markdown(&root);
    fs::write(&summary_markdown, markdown).wrap_err_with(|| {
        format!(
            "failed to write gateway M3 markdown `{}`",
            summary_markdown.display()
        )
    })?;

    Ok(GatewayM3Outcome {
        summary_json,
        summary_markdown,
    })
}

fn file_blake3_hex(path: &Path) -> Result<String> {
    let mut hasher = Blake3::new();
    let mut file = fs::File::open(path).wrap_err_with(|| format!("open {}", path.display()))?;
    std::io::copy(&mut file, &mut hasher).wrap_err_with(|| format!("hash {}", path.display()))?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn relative(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(stripped) => stripped.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

fn render_markdown(root: &Map) -> String {
    let autoscale = root
        .get("autoscale_plan")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let autoscale_hash = root
        .get("autoscale_plan_blake3")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let worker = root
        .get("worker_pack")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let worker_hash = root
        .get("worker_pack_blake3")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let sla = root
        .get("sla_target")
        .and_then(Value::as_str)
        .unwrap_or("99.95% regional");

    format!(
        "# Gateway GA readiness\n\
Autoscale plan: {autoscale} (blake3 {autoscale_hash})\n\
Worker pack: {worker} (blake3 {worker_hash})\n\
SLA target: {sla}\n"
    )
}
