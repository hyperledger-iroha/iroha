//! Hardening and privacy baseline checks for the SoraGlobal Gateway CDN (SNNet-15H).
//! Produces JSON/Markdown evidence that SBOMs, vuln scans, retention defaults,
//! and sandbox/HSM artefacts are present before promotion.

use std::{
    fs,
    path::{Path, PathBuf},
};

use blake3::Hasher as Blake3;
use eyre::{Result, WrapErr};
use norito::json::{self, Map, Number, Value};

/// Options for the hardening auditor.
#[derive(Debug, Clone)]
pub struct GatewayHardeningOptions {
    /// Output directory for summary artefacts.
    pub output_dir: PathBuf,
    /// Path to the SBOM attestation or archive.
    pub sbom: Option<PathBuf>,
    /// Path to the vulnerability scan report.
    pub vuln_report: Option<PathBuf>,
    /// Path to the HSM policy document.
    pub hsm_policy: Option<PathBuf>,
    /// Path to the sandbox profile (cgroup/WASM/runtime).
    pub sandbox_profile: Option<PathBuf>,
    /// Default data retention in days.
    pub data_retention_days: u32,
    /// Default log retention in days.
    pub log_retention_days: u32,
}

/// Locations of the produced summaries.
#[derive(Debug)]
pub struct GatewayHardeningOutcome {
    /// JSON summary path.
    pub summary_json: PathBuf,
    /// Markdown summary path.
    pub summary_markdown: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComponentState {
    Ok,
    Warn,
    Error,
}

impl ComponentState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn elevate(self, other: ComponentState) -> ComponentState {
        match (self, other) {
            (ComponentState::Error, _) | (_, ComponentState::Error) => ComponentState::Error,
            (ComponentState::Warn, _) | (_, ComponentState::Warn) => ComponentState::Warn,
            _ => ComponentState::Ok,
        }
    }
}

/// Run the hardening auditor and write JSON/Markdown summaries.
pub fn run_gateway_hardening(options: GatewayHardeningOptions) -> Result<GatewayHardeningOutcome> {
    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create hardening output dir `{}`",
            options.output_dir.display()
        )
    })?;

    let mut overall = ComponentState::Ok;
    let mut root = Map::new();

    let (sbom_state, sbom_obj) = check_file("sbom", options.sbom.as_deref())?;
    overall = overall.elevate(sbom_state);
    root.insert("sbom".into(), sbom_obj);

    let (vuln_state, vuln_obj) = check_file("vuln_report", options.vuln_report.as_deref())?;
    overall = overall.elevate(vuln_state);
    root.insert("vuln_report".into(), vuln_obj);

    let (hsm_state, hsm_obj) = check_file("hsm_policy", options.hsm_policy.as_deref())?;
    overall = overall.elevate(hsm_state);
    root.insert("hsm_policy".into(), hsm_obj);

    let (sandbox_state, sandbox_obj) =
        check_file("sandbox_profile", options.sandbox_profile.as_deref())?;
    overall = overall.elevate(sandbox_state);
    root.insert("sandbox_profile".into(), sandbox_obj);

    let retention_obj = retention_block(
        options.data_retention_days,
        options.log_retention_days,
        &mut overall,
    );
    root.insert("retention".into(), retention_obj);

    root.insert(
        "overall_status".into(),
        Value::String(overall.as_str().to_string()),
    );

    let summary_json = options.output_dir.join("gateway_hardening_summary.json");
    let summary_markdown = options.output_dir.join("gateway_hardening_summary.md");

    let json_payload = Value::Object(root.clone());
    let file = fs::File::create(&summary_json)
        .wrap_err_with(|| format!("create {}", summary_json.display()))?;
    json::to_writer_pretty(file, &json_payload)
        .wrap_err_with(|| format!("write {}", summary_json.display()))?;

    let markdown = render_markdown(&root);
    fs::write(&summary_markdown, markdown).wrap_err_with(|| {
        format!(
            "failed to write hardening markdown `{}`",
            summary_markdown.display()
        )
    })?;

    Ok(GatewayHardeningOutcome {
        summary_json,
        summary_markdown,
    })
}

fn check_file(label: &str, path: Option<&Path>) -> Result<(ComponentState, Value)> {
    let mut state = ComponentState::Ok;
    let mut obj = Map::new();
    if let Some(path) = path {
        let exists = path.exists();
        obj.insert("path".into(), Value::String(path.display().to_string()));
        obj.insert("present".into(), Value::Bool(exists));
        if exists {
            obj.insert("blake3_hex".into(), Value::String(file_blake3_hex(path)?));
        } else {
            state = ComponentState::Error;
            obj.insert(
                "error".into(),
                Value::String(format!("{label} missing at {}", path.display())),
            );
        }
    } else {
        state = ComponentState::Warn;
        obj.insert("present".into(), Value::Bool(false));
        obj.insert(
            "error".into(),
            Value::String(format!("{label} path not supplied")),
        );
    }
    Ok((state, Value::Object(obj)))
}

fn retention_block(data_days: u32, log_days: u32, overall: &mut ComponentState) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "data_retention_days".into(),
        Value::Number(Number::from(u64::from(data_days))),
    );
    obj.insert(
        "log_retention_days".into(),
        Value::Number(Number::from(u64::from(log_days))),
    );

    let mut state = ComponentState::Ok;
    if data_days > 30 || log_days > 30 {
        state = ComponentState::Warn;
    }
    obj.insert("state".into(), Value::String(state.as_str().to_string()));
    *overall = overall.elevate(state);
    Value::Object(obj)
}

fn file_blake3_hex(path: &Path) -> Result<String> {
    let mut hasher = Blake3::new();
    let mut file = fs::File::open(path).wrap_err_with(|| format!("open {}", path.display()))?;
    std::io::copy(&mut file, &mut hasher).wrap_err_with(|| format!("hash {}", path.display()))?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn render_markdown(root: &Map) -> String {
    let overall = root
        .get("overall_status")
        .and_then(Value::as_str)
        .unwrap_or("error");
    let sbom = component_state(root, "sbom");
    let vuln = component_state(root, "vuln_report");
    let hsm = component_state(root, "hsm_policy");
    let sandbox = component_state(root, "sandbox_profile");
    let retention = component_state(root, "retention");

    format!(
        "# Gateway hardening summary\n\
Overall status: {overall}\n\n\
- SBOM: {sbom}\n\
- Vulnerability report: {vuln}\n\
- HSM policy: {hsm}\n\
- Sandbox profile: {sandbox}\n\
- Retention: {retention}\n"
    )
}

fn component_state(root: &Map, key: &str) -> String {
    root.get(key)
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("state").and_then(Value::as_str))
        .unwrap_or("error")
        .to_string()
}
