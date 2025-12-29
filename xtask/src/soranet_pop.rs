//! Helpers for generating FRR configuration snippets for SoraNet PoPs and
//! provisioning bundles for PoP bring-up.
//!
//! Roadmap reference: **SNNet-15A2 – BGP policy & monitoring suite** and
//! **SN15-M0-2 – PoP automation toolchain bootstrap**.
//! The generator lets operators feed structured Norito JSON describing a PoP
//! into `cargo xtask soranet-pop-template` and receive a deterministic FRR
//! configuration with BFD/RPKI guards pre-wired.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt::Write,
    fs::{self, File},
    io::Read,
    net::IpAddr,
    path::{Path, PathBuf},
    process::Command,
};

use eyre::{Result, WrapErr, eyre};
use hex::encode as hex_encode;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::{self, Map, Value},
};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::workspace_root;

/// Options for producing a PoP provisioning bundle.
#[derive(Debug)]
pub struct PopBundleOptions {
    pub descriptor: PathBuf,
    pub roa_bundle: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub edns_resolver: Option<String>,
    pub edns_tool: Option<String>,
    pub skip_edns: bool,
    pub skip_ds: bool,
    pub image_tag: String,
}

/// Options for emitting the BGP policy + monitoring harness.
#[derive(Debug)]
pub struct PopPolicyHarnessOptions {
    pub descriptor: PathBuf,
    pub roa_bundle: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub grafana_out: Option<PathBuf>,
    pub alert_rules_out: Option<PathBuf>,
    pub report_out: Option<PathBuf>,
}

/// Bundle manifest describing the generated provisioning assets.
#[derive(Debug, JsonSerialize)]
pub struct PopBundleManifest {
    pop_name: String,
    descriptor_sha256: String,
    generated_at_rfc3339: String,
    frr_config: ArtifactRecord,
    resolver_config: ArtifactRecord,
    #[norito(default)]
    ds_validation: Option<ArtifactRecord>,
    #[norito(default)]
    edns_matrix: Option<ArtifactRecord>,
    #[norito(default)]
    roa_source: Option<String>,
    pxe_profile: ArtifactRecord,
    health_probe: ArtifactRecord,
    pop_env: ArtifactRecord,
    secrets_template: ArtifactRecord,
    bringup_checklist: ArtifactRecord,
    ci_job: ArtifactRecord,
    signoff_bundle: ArtifactRecord,
    sigstore_provenance_path: String,
    promotion_gates: Vec<String>,
    image_tag: String,
}

impl PopBundleManifest {
    fn save(&self, target: &Path) -> Result<()> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create bundle manifest parent {}",
                    parent.display()
                )
            })?;
        }
        let file = File::create(target)
            .wrap_err_with(|| format!("failed to create bundle manifest {}", target.display()))?;
        json::to_writer_pretty(file, self).wrap_err_with(|| {
            format!("failed to serialize bundle manifest {}", target.display())
        })?;
        Ok(())
    }
}

#[derive(Debug, Clone, JsonSerialize)]
struct ArtifactRecord {
    path: String,
    sha256: String,
    bytes: u64,
}

impl ArtifactRecord {
    fn from_path(path: &Path) -> Result<Self> {
        let metadata = fs::metadata(path)
            .wrap_err_with(|| format!("failed to read metadata for artifact {}", path.display()))?;
        let sha256 = sha256_file(path)?;
        Ok(Self {
            path: path.display().to_string(),
            sha256,
            bytes: metadata.len(),
        })
    }
}

#[derive(Debug, JsonSerialize)]
struct SignoffBundle {
    pop_name: String,
    image_tag: String,
    descriptor_sha256: String,
    generated_at_rfc3339: String,
    artifacts: Vec<ArtifactRecord>,
    promotion_gates: Vec<String>,
    sigstore_provenance_path: String,
}

/// Output of `build_pop_bundle`.
#[derive(Debug)]
pub struct PopBundleOutcome {
    #[allow(dead_code)]
    pub manifest: PopBundleManifest,
    pub manifest_path: PathBuf,
}

impl PopBundleOutcome {
    pub fn summary(&self) -> (&str, &str) {
        (&self.manifest.pop_name, self.manifest.image_tag.as_str())
    }
}

/// Output of the policy harness generator.
#[derive(Debug)]
#[allow(dead_code)]
pub struct PopPolicyHarnessOutcome {
    pub report_path: PathBuf,
    pub alert_rules_path: PathBuf,
    pub grafana_dashboard_path: PathBuf,
}

/// Generate an FRR configuration from a Norito JSON descriptor.
pub fn render_frr_config(input: &Path, output: &Path) -> Result<()> {
    let bytes = fs::read(input)
        .wrap_err_with(|| format!("failed to read PoP descriptor `{}`", input.display()))?;
    let descriptor: PopDescriptor = json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to parse PoP descriptor `{}` as Norito JSON",
            input.display()
        )
    })?;
    descriptor.validate()?;
    let rendered = descriptor.render_config()?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create parent directory for `{}`",
                output.display()
            )
        })?;
    }
    fs::write(output, rendered).wrap_err_with(|| {
        format!(
            "failed to write rendered FRR config to `{}`",
            output.display()
        )
    })?;
    Ok(())
}

/// Render a resolver/authoritative config template aligned with SNNet-15B.
pub fn render_resolver_config(input: &Path, output: &Path) -> Result<()> {
    let bytes = fs::read(input)
        .wrap_err_with(|| format!("failed to read PoP descriptor `{}`", input.display()))?;
    let descriptor: PopDescriptor = json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to parse PoP descriptor `{}` as Norito JSON",
            input.display()
        )
    })?;
    descriptor.validate()?;

    let zones_catalog = workspace_root().join("fixtures/soradns/managed_zones.sample.json");
    if !zones_catalog.exists() {
        return Err(eyre!(
            "managed zones catalog missing at {}; run `scripts/soradns_validation.py ds-validate` after adding it",
            zones_catalog.display()
        ));
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create parent directory for `{}`",
                output.display()
            )
        })?;
    }

    let contents = format!(
        "# Autogenerated resolver template for {pop}\n\
[resolver]\n\
pop = \"{pop}\"\n\
managed_zones_catalog = \"{zones}\"\n\
protocols = [\"doh\",\"dot\",\"doq\",\"odoh-preview\"]\n\
ingress_mtls_required = true\n\
\n\
[resolver.policy]\n\
qname_minimisation = \"default\"\n\
ecs_mode = \"opt-in\"\n\
serve_stale_seconds = 1800\n\
\n\
[resolver.odoh]\n\
relay_key_path = \"artifacts/soradns/relays/{pop}/relay.keys\"\n\
target_upstream = \"https://127.0.0.1:443/dns-query\"\n\
\n\
[resolver.verification_receipt]\n\
zrh_template = \"artifacts/soradns/proofs/{pop}/{{zone}}/vr.json\"\n\
\n\
# Update the authoritative shim once staging proofs are available\n\
[authoritative]\n\
zone_root_path = \"artifacts/soradns/proofs/{pop}/{{zone}}/proof.json\"\n\
tls_profile = \"default\"\n",
        pop = descriptor.pop_name,
        zones = zones_catalog.display()
    );

    fs::write(output, contents)
        .wrap_err_with(|| format!("failed to write resolver config to `{}`", output.display()))?;
    Ok(())
}

/// Run the EDNS matrix helper to capture resolver behaviour for GAR packets.
pub fn run_edns_matrix(resolver: &str, output: &Path, tool: Option<&str>) -> Result<()> {
    let script = workspace_root().join("scripts/soradns_validation.py");
    if !script.exists() {
        return Err(eyre!(
            "missing helper {}; ensure the repo is checked out correctly",
            script.display()
        ));
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create parent directory for `{}`",
                output.display()
            )
        })?;
    }

    let mut cmd = Command::new("python3");
    cmd.arg(script)
        .arg("edns-matrix")
        .arg("--resolver")
        .arg(resolver)
        .arg("--out")
        .arg(output);
    if let Some(bin) = tool {
        cmd.arg("--tool").arg(bin);
    }

    let status = cmd
        .status()
        .wrap_err("failed to run soradns_validation.py edns-matrix")?;
    if !status.success() {
        return Err(eyre!("soradns_validation.py exited with {}", status));
    }
    Ok(())
}

/// Run DS/DNSKEY validation for the managed zones catalog.
pub fn run_ds_validation(output: &Path) -> Result<()> {
    let script = workspace_root().join("scripts/soradns_validation.py");
    if !script.exists() {
        return Err(eyre!(
            "missing helper {}; ensure the repo is checked out correctly",
            script.display()
        ));
    }
    let zones_catalog = workspace_root().join("fixtures/soradns/managed_zones.sample.json");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create parent directory for `{}`",
                output.display()
            )
        })?;
    }

    let status = Command::new("python3")
        .arg(script)
        .arg("ds-validate")
        .arg("--zones")
        .arg(&zones_catalog)
        .arg("--out")
        .arg(output)
        .status()
        .wrap_err("failed to run soradns_validation.py ds-validate")?;
    if !status.success() {
        return Err(eyre!(
            "soradns_validation.py ds-validate exited with {}",
            status
        ));
    }
    Ok(())
}

/// Generate a full provisioning bundle for a PoP, including FRR config,
/// resolver template, optional EDNS/DS evidence, PXE profile stub, health probe,
/// and a manifest with checksums and timestamps.
pub fn build_pop_bundle(options: PopBundleOptions) -> Result<PopBundleOutcome> {
    let PopBundleOptions {
        descriptor,
        roa_bundle,
        output_dir,
        edns_resolver,
        edns_tool,
        skip_edns,
        skip_ds,
        image_tag,
    } = options;

    let descriptor_sha256 = sha256_file(&descriptor)?;
    let descriptor_bytes = fs::read(&descriptor)
        .wrap_err_with(|| format!("failed to read PoP descriptor `{}`", descriptor.display()))?;
    let pop: PopDescriptor = json::from_slice(&descriptor_bytes).wrap_err_with(|| {
        format!(
            "failed to parse PoP descriptor `{}` as Norito JSON",
            descriptor.display()
        )
    })?;
    pop.validate()?;
    let pop_name = pop.pop_name.clone();
    let bundle_root = output_dir;
    fs::create_dir_all(&bundle_root).wrap_err_with(|| {
        format!(
            "failed to create bundle directory `{}`",
            bundle_root.display()
        )
    })?;
    let generated_at = OffsetDateTime::now_utc();
    let generated_at_rfc3339 = generated_at
        .format(&Rfc3339)
        .wrap_err("failed to format manifest timestamp")?;

    let frr_path = bundle_root.join("frr.conf");
    render_frr_config(&descriptor, &frr_path)?;

    let resolver_path = bundle_root.join("resolver.toml");
    render_resolver_config(&descriptor, &resolver_path)?;

    let ds_path = if skip_ds {
        None
    } else {
        let path = bundle_root.join("ds_validation.json");
        run_ds_validation(&path)?;
        Some(path)
    };

    let edns_path = if skip_edns {
        None
    } else {
        let path = bundle_root.join("edns_matrix.json");
        let resolver_addr = edns_resolver.unwrap_or_else(|| "127.0.0.1".to_string());
        run_edns_matrix(&resolver_addr, &path, edns_tool.as_deref())?;
        Some(path)
    };

    // Validate descriptor/ROA coverage to keep bundle generation honest.
    validate_pop_descriptor(&descriptor, roa_bundle.as_deref())?;

    let pxe_profile = write_pxe_profile_stub(&pop_name, &bundle_root)?;
    let health_probe = write_health_probe_stub(&pop, &bundle_root)?;
    let pop_env = write_pop_env_template(&pop, &bundle_root, &image_tag)?;
    let secrets_template = write_secrets_template(&pop, &bundle_root)?;
    let bringup_checklist = write_bringup_checklist(&pop, &bundle_root, &image_tag)?;
    let ci_job = write_ci_stub(
        &pop,
        &bundle_root,
        &descriptor,
        roa_bundle.as_deref(),
        &image_tag,
    )?;

    let frr_config = ArtifactRecord::from_path(&frr_path)?;
    let resolver_config = ArtifactRecord::from_path(&resolver_path)?;
    let ds_validation = match ds_path {
        Some(path) => Some(ArtifactRecord::from_path(&path)?),
        None => None,
    };
    let edns_matrix = match edns_path {
        Some(path) => Some(ArtifactRecord::from_path(&path)?),
        None => None,
    };
    let pxe_profile = ArtifactRecord::from_path(&pxe_profile)?;
    let health_probe = ArtifactRecord::from_path(&health_probe)?;
    let pop_env = ArtifactRecord::from_path(&pop_env)?;
    let secrets_template = ArtifactRecord::from_path(&secrets_template)?;
    let bringup_checklist = ArtifactRecord::from_path(&bringup_checklist)?;
    let ci_job = ArtifactRecord::from_path(&ci_job)?;
    let promotion_gates = promotion_gate_messages(&pop_name, &image_tag);

    let signoff_dir = bundle_root.join("attestations");
    fs::create_dir_all(&signoff_dir).wrap_err_with(|| {
        format!(
            "failed to create signoff directory `{}`",
            signoff_dir.display()
        )
    })?;
    let signoff_path = signoff_dir.join("signoff.json");
    let sigstore_path = signoff_dir.join(format!(
        "soranet-{}-provenance.intoto.jsonl",
        sanitize_label(&pop_name)
    ));
    let mut signoff_artifacts = vec![
        frr_config.clone(),
        resolver_config.clone(),
        pxe_profile.clone(),
        health_probe.clone(),
        pop_env.clone(),
        secrets_template.clone(),
        bringup_checklist.clone(),
        ci_job.clone(),
    ];
    if let Some(record) = ds_validation.as_ref() {
        signoff_artifacts.push(record.clone());
    }
    if let Some(record) = edns_matrix.as_ref() {
        signoff_artifacts.push(record.clone());
    }
    write_sigstore_stub(&sigstore_path, &pop_name, &image_tag)?;
    let sigstore_stub = ArtifactRecord::from_path(&sigstore_path)?;
    signoff_artifacts.push(sigstore_stub.clone());

    let signoff = SignoffBundle {
        pop_name: pop_name.clone(),
        image_tag: image_tag.clone(),
        descriptor_sha256: descriptor_sha256.clone(),
        generated_at_rfc3339: generated_at_rfc3339.clone(),
        artifacts: signoff_artifacts,
        promotion_gates: promotion_gates.clone(),
        sigstore_provenance_path: sigstore_path.display().to_string(),
    };
    write_signoff_bundle(&signoff_path, &signoff)?;
    let signoff_bundle = ArtifactRecord::from_path(&signoff_path)?;

    let manifest = PopBundleManifest {
        pop_name,
        descriptor_sha256,
        generated_at_rfc3339,
        frr_config,
        resolver_config,
        ds_validation,
        edns_matrix,
        roa_source: roa_bundle.as_ref().map(|path| path.display().to_string()),
        pxe_profile,
        health_probe,
        pop_env,
        secrets_template,
        bringup_checklist,
        ci_job,
        signoff_bundle,
        sigstore_provenance_path: sigstore_stub.path,
        promotion_gates,
        image_tag,
    };

    let manifest_path = bundle_root.join("bundle_manifest.json");
    manifest.save(&manifest_path)?;

    Ok(PopBundleOutcome {
        manifest,
        manifest_path,
    })
}

/// Emit alert rules, Grafana dashboard scaffolding, and a JSON summary for a PoP.
pub fn build_pop_policy_harness(
    options: PopPolicyHarnessOptions,
) -> Result<PopPolicyHarnessOutcome> {
    let PopPolicyHarnessOptions {
        descriptor,
        roa_bundle,
        output_dir,
        grafana_out,
        alert_rules_out,
        report_out,
    } = options;

    let descriptor_bytes = fs::read(&descriptor)
        .wrap_err_with(|| format!("failed to read PoP descriptor `{}`", descriptor.display()))?;
    let pop: PopDescriptor = json::from_slice(&descriptor_bytes).wrap_err_with(|| {
        format!(
            "failed to parse PoP descriptor `{}` as Norito JSON",
            descriptor.display()
        )
    })?;
    pop.validate()?;

    fs::create_dir_all(&output_dir).wrap_err_with(|| {
        format!(
            "failed to create policy harness directory `{}`",
            output_dir.display()
        )
    })?;
    let grafana_dashboard_path =
        grafana_out.unwrap_or_else(|| output_dir.join("grafana_soranet_bgp.json"));
    let alert_rules_path =
        alert_rules_out.unwrap_or_else(|| output_dir.join("bgp_alert_rules.yml"));
    let report_path = report_out.unwrap_or_else(|| output_dir.join("policy_report.json"));

    let grafana_dashboard = render_bgp_grafana_dashboard(&pop);
    if let Some(parent) = grafana_dashboard_path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create grafana parent directory `{}`",
                parent.display()
            )
        })?;
    }
    let grafana_json = json::to_string_pretty(&grafana_dashboard).wrap_err_with(|| {
        format!(
            "failed to serialize grafana dashboard for pop {}",
            pop.pop_name
        )
    })?;
    fs::write(&grafana_dashboard_path, grafana_json).wrap_err_with(|| {
        format!(
            "failed to write grafana dashboard to {}",
            grafana_dashboard_path.display()
        )
    })?;

    if let Some(parent) = alert_rules_path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create alert rules parent directory `{}`",
                parent.display()
            )
        })?;
    }
    let alert_rules = render_bgp_alert_rules(&pop);
    fs::write(&alert_rules_path, alert_rules).wrap_err_with(|| {
        format!(
            "failed to write alert rules to {}",
            alert_rules_path.display()
        )
    })?;

    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create policy report parent directory `{}`",
                parent.display()
            )
        })?;
    }

    let validation = validate_pop_descriptor(&descriptor, roa_bundle.as_deref())?;
    let bfd_plan = pop.bfd_plan();
    let route_health = RouteHealthTargets {
        expected_neighbors: pop.neighbors.len(),
        expected_prefixes: pop.prefixes.all().len(),
        expected_rpki_caches: pop.rpki_caches.len(),
        expected_bfd_sessions: bfd_plan.expected_sessions(),
        local_pref_targets: pop
            .neighbors
            .iter()
            .enumerate()
            .filter_map(|(idx, neighbor)| {
                neighbor.local_pref.map(|value| NeighborValueTarget {
                    neighbor: neighbor.name.clone(),
                    address: neighbor.address.clone(),
                    value: value as i64,
                    index: idx as u64,
                })
            })
            .collect(),
        med_targets: pop
            .neighbors
            .iter()
            .enumerate()
            .filter_map(|(idx, neighbor)| {
                neighbor.med.map(|value| NeighborValueTarget {
                    neighbor: neighbor.name.clone(),
                    address: neighbor.address.clone(),
                    value: value as i64,
                    index: idx as u64,
                })
            })
            .collect(),
    };
    let report = PopPolicyHarnessReport {
        pop_name: pop.pop_name.clone(),
        alert_rules_path: alert_rules_path.display().to_string(),
        grafana_dashboard_path: grafana_dashboard_path.display().to_string(),
        route_health,
        validation,
    };

    let file = File::create(&report_path)
        .wrap_err_with(|| format!("failed to create policy report {}", report_path.display()))?;
    json::to_writer_pretty(file, &report)
        .wrap_err_with(|| format!("failed to write policy report to {}", report_path.display()))?;

    Ok(PopPolicyHarnessOutcome {
        report_path,
        alert_rules_path,
        grafana_dashboard_path,
    })
}

/// Validate a PoP descriptor and optional ROA bundle, returning a structured report.
pub fn validate_pop_descriptor(
    input: &Path,
    roa_bundle: Option<&Path>,
) -> Result<PopValidationReport> {
    let bytes = fs::read(input)
        .wrap_err_with(|| format!("failed to read PoP descriptor `{}`", input.display()))?;
    let descriptor: PopDescriptor = json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to parse PoP descriptor `{}` as Norito JSON",
            input.display()
        )
    })?;
    descriptor.validate()?;
    let bfd_plan = descriptor.bfd_plan();

    let roa_bundle = match roa_bundle {
        Some(path) => Some(RoaBundle::load(path)?),
        None => None,
    };

    let prefixes_without_roa = if let Some(bundle) = &roa_bundle {
        descriptor
            .prefixes
            .all()
            .into_iter()
            .filter(|prefix| !bundle.matches(prefix, descriptor.asn))
            .collect()
    } else {
        Vec::new()
    };

    let mut rpki_cache_warnings = Vec::new();
    if descriptor.rpki_caches.is_empty() {
        rpki_cache_warnings.push(
            "no RPKI cache configured; enable rpki cache <host> <port> and logging".to_string(),
        );
    }
    let (communities, using_default_communities) = descriptor.resolved_communities();

    let report = PopValidationReport {
        pop_name: descriptor.pop_name.clone(),
        asn: descriptor.asn,
        rpki_caches: descriptor.rpki_caches.len(),
        has_rpki_cache: !descriptor.rpki_caches.is_empty(),
        rpki_cache_warnings,
        neighbors_total: descriptor.neighbors.len(),
        missing_bfd_neighbors: bfd_plan.autofilled_neighbors.clone(),
        bfd_autofill_neighbors: bfd_plan.autofilled_neighbors.clone(),
        prefixes_without_roa,
        roa_source: roa_bundle.and_then(|bundle| bundle.source),
        recommended_bfd_profile: bfd_plan
            .recommendations
            .first()
            .cloned()
            .unwrap_or_else(recommended_bfd_profile),
        bfd_recommendations: bfd_plan.recommendations.clone(),
        rpki_logging_recommended: true,
        communities: CommunityReport::from_values(&communities, using_default_communities),
    };
    Ok(report)
}

/// Validation report to surface gaps and defaults for operators.
#[derive(Debug, JsonSerialize)]
pub struct PopValidationReport {
    pop_name: String,
    asn: u32,
    rpki_caches: usize,
    has_rpki_cache: bool,
    rpki_cache_warnings: Vec<String>,
    neighbors_total: usize,
    missing_bfd_neighbors: Vec<String>,
    #[norito(default)]
    bfd_autofill_neighbors: Vec<String>,
    prefixes_without_roa: Vec<String>,
    #[norito(default)]
    roa_source: Option<String>,
    recommended_bfd_profile: BfdRecommendation,
    #[norito(default)]
    bfd_recommendations: Vec<BfdRecommendation>,
    rpki_logging_recommended: bool,
    communities: CommunityReport,
}

impl PopValidationReport {
    pub fn to_value(&self) -> Value {
        json::to_value(self).expect("serialize PoP validation report")
    }
}

#[derive(Debug, JsonSerialize)]
struct PopPolicyHarnessReport {
    pop_name: String,
    alert_rules_path: String,
    grafana_dashboard_path: String,
    route_health: RouteHealthTargets,
    validation: PopValidationReport,
}

#[derive(Debug, JsonSerialize)]
struct RouteHealthTargets {
    expected_neighbors: usize,
    expected_prefixes: usize,
    expected_rpki_caches: usize,
    expected_bfd_sessions: usize,
    #[norito(default)]
    local_pref_targets: Vec<NeighborValueTarget>,
    #[norito(default)]
    med_targets: Vec<NeighborValueTarget>,
}

#[derive(Debug, JsonSerialize)]
struct NeighborValueTarget {
    neighbor: String,
    address: String,
    value: i64,
    index: u64,
}

#[derive(Debug, JsonSerialize)]
struct CommunityReport {
    drain: String,
    fail_open: String,
    blackhole: String,
    using_defaults: bool,
}

impl CommunityReport {
    fn from_values(values: &CommunityDefaults, using_defaults: bool) -> Self {
        Self {
            drain: values.drain.clone(),
            fail_open: values.fail_open.clone(),
            blackhole: values.blackhole.clone(),
            using_defaults,
        }
    }
}

#[derive(Debug, JsonDeserialize)]
struct PopDescriptor {
    pop_name: String,
    asn: u32,
    router_id: String,
    loopback_ipv4: String,
    #[norito(default)]
    loopback_ipv6: Option<String>,
    #[norito(default)]
    rpki_caches: Vec<RpkiCache>,
    #[norito(default)]
    bfd_profiles: Vec<BfdProfile>,
    #[norito(default)]
    prefixes: PrefixCollection,
    #[norito(default)]
    communities: Option<CommunityConfig>,
    neighbors: Vec<PopNeighbor>,
}

impl PopDescriptor {
    fn validate(&self) -> Result<()> {
        if self.pop_name.trim().is_empty() {
            return Err(eyre!("pop_name must not be empty"));
        }
        if self.asn == 0 {
            return Err(eyre!("asn must not be zero"));
        }
        ensure_ipv4("router_id", &self.router_id)?;
        ensure_ipv4("loopback_ipv4", &self.loopback_ipv4)?;
        if let Some(loopback) = &self.loopback_ipv6 {
            ensure_ipv6("loopback_ipv6", loopback)?;
        }
        for prefix in self.prefixes.all() {
            ensure_prefix(&prefix)?;
        }
        if let Some(communities) = &self.communities {
            communities.validate()?;
        }
        if self.neighbors.is_empty() {
            return Err(eyre!("PoP descriptor must contain at least one neighbor"));
        }
        let profile_names: HashSet<&str> =
            self.bfd_profiles.iter().map(|p| p.name.as_str()).collect();
        for neighbor in &self.neighbors {
            neighbor.validate(&profile_names)?;
        }
        Ok(())
    }

    fn resolved_communities(&self) -> (CommunityDefaults, bool) {
        match &self.communities {
            Some(config) => (config.to_defaults(), false),
            None => (CommunityDefaults::recommended(), true),
        }
    }

    fn bfd_plan(&self) -> BfdPlan {
        let mut profiles: BTreeMap<String, BfdProfile> = self
            .bfd_profiles
            .iter()
            .map(|p| (p.name.clone(), p.clone()))
            .collect();

        let mut neighbor_profiles = Vec::new();
        let mut autofilled_neighbors = Vec::new();

        for neighbor in &self.neighbors {
            if let Some(existing) = neighbor.bfd_profile.clone() {
                neighbor_profiles.push(Some(existing));
            } else {
                let rec = recommended_bfd_profile();
                if !profiles.contains_key(&rec.name) {
                    profiles.insert(rec.name.clone(), rec.to_profile());
                }
                neighbor_profiles.push(Some(rec.name.clone()));
                autofilled_neighbors.push(neighbor.name.clone());
            }
        }

        let recommendations = vec![recommended_bfd_profile()];

        BfdPlan {
            profiles,
            neighbor_profiles,
            autofilled_neighbors,
            recommendations,
        }
    }

    fn render_config(&self) -> Result<String> {
        let mut out = String::new();
        let (communities, using_default_communities) = self.resolved_communities();
        let bfd_plan = self.bfd_plan();
        writeln!(out, "frr defaults datacenter").unwrap();
        writeln!(out, "hostname sora-{}-edge", sanitize_label(&self.pop_name)).unwrap();
        writeln!(out, "log syslog informational").unwrap();
        writeln!(out, "service integrated-vtysh-config").unwrap();
        writeln!(out, "!").unwrap();

        if !bfd_plan.profiles.is_empty() {
            writeln!(out, "bfd").unwrap();
            for profile in bfd_plan.profiles.values() {
                writeln!(out, "  profile {}", profile.name).unwrap();
                writeln!(out, "    transmit-interval {}", profile.transmit_ms).unwrap();
                writeln!(out, "    receive-interval {}", profile.receive_ms).unwrap();
                writeln!(
                    out,
                    "    detect-multiplier {}",
                    profile.detect_multiplier.max(1)
                )
                .unwrap();
                writeln!(out, "  !").unwrap();
            }
            writeln!(out, "!").unwrap();
        }

        if !self.rpki_caches.is_empty() {
            writeln!(out, "rpki").unwrap();
            for cache in &self.rpki_caches {
                if let Some(desc) = cache.description.as_deref() {
                    writeln!(
                        out,
                        "  cache {} {} description \"{}\"",
                        cache.address, cache.port, desc
                    )
                    .unwrap();
                } else {
                    writeln!(out, "  cache {} {}", cache.address, cache.port).unwrap();
                }
                if let Some(pref) = cache.preference {
                    writeln!(
                        out,
                        "  cache {} {} preference {}",
                        cache.address, cache.port, pref
                    )
                    .unwrap();
                }
            }
            writeln!(out, "  logging").unwrap();
            writeln!(out, "!").unwrap();
        }

        writeln!(
            out,
            "ip community-list standard SORANET-DRAIN permit {}",
            communities.drain
        )
        .unwrap();
        writeln!(
            out,
            "ip community-list standard SORANET-FAIL-OPEN permit {}",
            communities.fail_open
        )
        .unwrap();
        writeln!(
            out,
            "ip community-list standard SORANET-BLACKHOLE permit {}",
            communities.blackhole
        )
        .unwrap();
        writeln!(out, "!").unwrap();

        let mut import_route_maps = Vec::new();
        let mut export_route_maps = Vec::new();

        writeln!(out, "router bgp {}", self.asn).unwrap();
        writeln!(out, "  bgp router-id {}", self.router_id).unwrap();
        writeln!(out, "  no bgp default ipv4-unicast").unwrap();
        writeln!(out, "  bgp deterministic-med").unwrap();
        writeln!(out, "  bgp bestpath as-path multipath-relax").unwrap();
        writeln!(out, "  timers bgp 3 9").unwrap();

        for (idx, neighbor) in self.neighbors.iter().enumerate() {
            let neighbor_id = &neighbor.address;
            writeln!(
                out,
                "  neighbor {neighbor_id} remote-as {}",
                neighbor.peer_asn
            )
            .unwrap();
            if let Some(desc) = neighbor.description.as_deref() {
                writeln!(out, "  neighbor {neighbor_id} description {}", desc).unwrap();
            }
            if let Some(profile) = bfd_plan.profile_for_neighbor(idx) {
                writeln!(out, "  neighbor {neighbor_id} bfd profile {profile}").unwrap();
            }
            let timers = neighbor.timers();
            writeln!(
                out,
                "  neighbor {neighbor_id} timers {} {}",
                timers.keepalive, timers.hold
            )
            .unwrap();
            if let Some(ttl) = neighbor.multihop_ttl {
                writeln!(out, "  neighbor {neighbor_id} ebgp-multihop {}", ttl.max(1)).unwrap();
            }
            if neighbor.send_community_both {
                writeln!(out, "  neighbor {neighbor_id} send-community both").unwrap();
            }
            if let Some(max_prefix) = neighbor.max_prefix {
                writeln!(
                    out,
                    "  neighbor {neighbor_id} maximum-prefix {} 80 restart 5",
                    max_prefix.max(1)
                )
                .unwrap();
            }
            if let Some(password) = neighbor.password.as_deref() {
                writeln!(out, "  neighbor {neighbor_id} password {}", password).unwrap();
            }

            if neighbor.requires_import_map() {
                let map_name = neighbor.import_route_map_name();
                import_route_maps.push(neighbor.build_import_route_map(&map_name));
                writeln!(out, "  neighbor {neighbor_id} route-map {map_name} in").unwrap();
            }
            if neighbor.requires_export_map() {
                let map_name = neighbor.export_route_map_name();
                export_route_maps.push(neighbor.build_export_route_map(&map_name));
                writeln!(out, "  neighbor {neighbor_id} route-map {map_name} out").unwrap();
            }
        }

        if !self.prefixes.ipv4.is_empty() {
            writeln!(out, "  address-family ipv4 unicast").unwrap();
            writeln!(out, "    network {}", self.loopback_ipv4).unwrap();
            for prefix in &self.prefixes.ipv4 {
                writeln!(out, "    network {}", prefix).unwrap();
            }
            for neighbor in self
                .neighbors
                .iter()
                .filter(|n| n.family() == AddressFamily::Ipv4)
            {
                writeln!(out, "    neighbor {} activate", neighbor.address).unwrap();
            }
            writeln!(out, "  exit-address-family").unwrap();
        }

        if !self.prefixes.ipv6.is_empty() || self.loopback_ipv6.is_some() {
            writeln!(out, "  address-family ipv6 unicast").unwrap();
            if let Some(loopback) = &self.loopback_ipv6 {
                writeln!(out, "    network {}", loopback).unwrap();
            }
            for prefix in &self.prefixes.ipv6 {
                writeln!(out, "    network {}", prefix).unwrap();
            }
            for neighbor in self
                .neighbors
                .iter()
                .filter(|n| n.family() == AddressFamily::Ipv6)
            {
                writeln!(out, "    neighbor {} activate", neighbor.address).unwrap();
            }
            writeln!(out, "  exit-address-family").unwrap();
        }
        writeln!(out, "!").unwrap();

        if !import_route_maps.is_empty() || !export_route_maps.is_empty() {
            writeln!(out, "! Route-map definitions").unwrap();
            for map in import_route_maps.iter().chain(export_route_maps.iter()) {
                out.push_str(map);
                if !map.ends_with('\n') {
                    out.push('\n');
                }
            }
        }

        let mut metadata = Map::new();
        metadata.insert("pop_name".into(), Value::String(self.pop_name.clone()));
        metadata.insert(
            "neighbor_count".into(),
            Value::from(self.neighbors.len() as u64),
        );
        let bfd_profile_names: Vec<String> = bfd_plan.profiles.keys().cloned().collect();
        metadata.insert(
            "bfd_profiles".into(),
            json::to_value(&bfd_profile_names).unwrap(),
        );
        metadata.insert(
            "bfd_autofill_neighbors".into(),
            json::to_value(&bfd_plan.autofilled_neighbors).unwrap(),
        );
        metadata.insert("communities".into(), json::to_value(&communities).unwrap());
        metadata.insert(
            "communities_from_descriptor".into(),
            Value::Bool(!using_default_communities),
        );
        writeln!(
            out,
            "! metadata: {}",
            json::to_string(&Value::Object(metadata)).unwrap()
        )
        .unwrap();

        Ok(out)
    }
}

fn ensure_ipv4(label: &str, value: &str) -> Result<()> {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => Ok(()),
        Ok(IpAddr::V6(_)) => Err(eyre!("{label} must be an IPv4 address, got IPv6: {value}")),
        Err(err) => Err(eyre!("invalid {label} `{value}`: {err}")),
    }
}

fn ensure_ipv6(label: &str, value: &str) -> Result<()> {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => Ok(()),
        Ok(IpAddr::V4(_)) => Err(eyre!("{label} must be an IPv6 address, got IPv4: {value}")),
        Err(err) => Err(eyre!("invalid {label} `{value}`: {err}")),
    }
}

fn ensure_prefix(prefix: &str) -> Result<()> {
    let (addr, len) = prefix
        .split_once('/')
        .ok_or_else(|| eyre!("prefix `{prefix}` is missing a slash"))?;
    let parsed: IpAddr = addr
        .parse()
        .wrap_err_with(|| format!("prefix `{prefix}` has an invalid IP"))?;
    let length: u8 = len
        .parse()
        .wrap_err_with(|| format!("prefix `{prefix}` has an invalid length"))?;
    let max_len = match parsed {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if length > max_len {
        return Err(eyre!(
            "prefix `{prefix}` length {} exceeds maximum {}",
            length,
            max_len
        ));
    }
    Ok(())
}

fn validate_community_value(kind: &str, value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("{kind} community must not be empty"));
    }
    let lower = trimmed.to_ascii_lowercase();
    let allowed_keywords = [
        "no-export",
        "no-advertise",
        "no-export-subconfed",
        "accept-own",
        "nopeer",
        "local-as",
        "graceful-shutdown",
        "blackhole",
        "internet",
    ];
    if allowed_keywords.contains(&lower.as_str()) {
        return Ok(());
    }
    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() == 2 || parts.len() == 3 {
        for part in &parts {
            part.parse::<u32>().wrap_err_with(|| {
                format!("{kind} community `{value}` uses non-numeric component `{part}`")
            })?;
        }
        return Ok(());
    }
    Err(eyre!(
        "{kind} community `{value}` must be colon-delimited numeric components or one of {:?}",
        allowed_keywords
    ))
}

#[derive(Debug, JsonDeserialize)]
struct RpkiCache {
    address: String,
    port: u16,
    #[norito(default)]
    description: Option<String>,
    #[norito(default)]
    preference: Option<u16>,
}

#[derive(Debug, Clone, JsonDeserialize)]
struct BfdProfile {
    name: String,
    transmit_ms: u32,
    receive_ms: u32,
    detect_multiplier: u8,
}

#[derive(Debug, Default, JsonDeserialize)]
struct PrefixCollection {
    #[norito(default)]
    ipv4: Vec<String>,
    #[norito(default)]
    ipv6: Vec<String>,
}

impl PrefixCollection {
    fn all(&self) -> Vec<String> {
        let mut set = BTreeSet::new();
        for prefix in &self.ipv4 {
            set.insert(prefix.clone());
        }
        for prefix in &self.ipv6 {
            set.insert(prefix.clone());
        }
        set.into_iter().collect()
    }
}

#[derive(Debug, Default, JsonDeserialize)]
struct CommunityConfig {
    #[norito(default)]
    drain: Option<String>,
    #[norito(default)]
    fail_open: Option<String>,
    #[norito(default)]
    blackhole: Option<String>,
}

impl CommunityConfig {
    fn validate(&self) -> Result<()> {
        if let Some(value) = &self.drain {
            validate_community_value("drain", value)?;
        }
        if let Some(value) = &self.fail_open {
            validate_community_value("fail_open", value)?;
        }
        if let Some(value) = &self.blackhole {
            validate_community_value("blackhole", value)?;
        }
        Ok(())
    }

    fn to_defaults(&self) -> CommunityDefaults {
        let defaults = CommunityDefaults::recommended();
        CommunityDefaults {
            drain: self.drain.clone().unwrap_or_else(|| defaults.drain.clone()),
            fail_open: self
                .fail_open
                .clone()
                .unwrap_or_else(|| defaults.fail_open.clone()),
            blackhole: self
                .blackhole
                .clone()
                .unwrap_or_else(|| defaults.blackhole.clone()),
        }
    }
}

#[derive(Debug, JsonDeserialize)]
struct PopNeighbor {
    name: String,
    address: String,
    peer_asn: u32,
    #[norito(default)]
    description: Option<String>,
    #[norito(default)]
    keepalive_secs: Option<u16>,
    #[norito(default)]
    hold_secs: Option<u16>,
    #[norito(default)]
    multihop_ttl: Option<u8>,
    #[norito(default)]
    bfd_profile: Option<String>,
    #[norito(default)]
    local_pref: Option<u32>,
    #[norito(default)]
    med: Option<i32>,
    #[norito(default)]
    import_rpki_deny_invalid: bool,
    #[norito(default)]
    max_prefix: Option<u32>,
    #[norito(default)]
    send_community_both: bool,
    #[norito(default)]
    password: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AddressFamily {
    Ipv4,
    Ipv6,
}

struct Timers {
    keepalive: u16,
    hold: u16,
}

impl PopNeighbor {
    fn validate(&self, profiles: &HashSet<&str>) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(eyre!("neighbor name must not be empty"));
        }
        if self.address.parse::<IpAddr>().is_err() {
            return Err(eyre!(
                "neighbor `{}` supplied invalid IP address `{}`",
                self.name,
                self.address
            ));
        }
        if let Some(profile) = self.bfd_profile.as_deref()
            && !profiles.contains(profile)
        {
            return Err(eyre!(
                "neighbor `{}` references unknown BFD profile `{profile}`",
                self.name
            ));
        }
        Ok(())
    }

    fn family(&self) -> AddressFamily {
        match self.address.parse::<IpAddr>() {
            Ok(IpAddr::V4(_)) => AddressFamily::Ipv4,
            _ => AddressFamily::Ipv6,
        }
    }

    fn timers(&self) -> Timers {
        let keepalive = self.keepalive_secs.unwrap_or(5).max(1);
        let hold = self.hold_secs.unwrap_or(15).max(keepalive + 1);
        Timers { keepalive, hold }
    }

    fn requires_import_map(&self) -> bool {
        self.local_pref.is_some() || self.import_rpki_deny_invalid
    }

    fn requires_export_map(&self) -> bool {
        self.med.is_some()
    }

    fn import_route_map_name(&self) -> String {
        format!("RM_{}_IMPORT", sanitize_label(&self.name))
    }

    fn export_route_map_name(&self) -> String {
        format!("RM_{}_EXPORT", sanitize_label(&self.name))
    }

    fn build_import_route_map(&self, name: &str) -> String {
        let mut buf = String::new();
        if self.import_rpki_deny_invalid {
            writeln!(buf, "route-map {name} deny 5").unwrap();
            writeln!(buf, "  match rpki invalid").unwrap();
            writeln!(buf, "!").unwrap();
        }
        if let Some(pref) = self.local_pref {
            writeln!(buf, "route-map {name} permit 10").unwrap();
            writeln!(buf, "  set local-preference {}", pref).unwrap();
            writeln!(buf, "!").unwrap();
        }
        buf
    }

    fn build_export_route_map(&self, name: &str) -> String {
        let mut buf = String::new();
        if let Some(med) = self.med {
            writeln!(buf, "route-map {name} permit 10").unwrap();
            writeln!(buf, "  set metric {}", med).unwrap();
            writeln!(buf, "!").unwrap();
        }
        buf
    }
}

#[derive(Debug, Clone, JsonSerialize)]
struct BfdRecommendation {
    name: String,
    transmit_ms: u32,
    receive_ms: u32,
    detect_multiplier: u8,
}

impl BfdRecommendation {
    fn to_profile(&self) -> BfdProfile {
        BfdProfile {
            name: self.name.clone(),
            transmit_ms: self.transmit_ms,
            receive_ms: self.receive_ms,
            detect_multiplier: self.detect_multiplier,
        }
    }
}

fn recommended_bfd_profile() -> BfdRecommendation {
    BfdRecommendation {
        name: "default-fast".to_string(),
        transmit_ms: 300,
        receive_ms: 300,
        detect_multiplier: 3,
    }
}

#[derive(Debug)]
struct BfdPlan {
    profiles: BTreeMap<String, BfdProfile>,
    neighbor_profiles: Vec<Option<String>>,
    autofilled_neighbors: Vec<String>,
    recommendations: Vec<BfdRecommendation>,
}

impl BfdPlan {
    fn profile_for_neighbor(&self, index: usize) -> Option<&str> {
        self.neighbor_profiles
            .get(index)
            .and_then(|profile| profile.as_deref())
    }

    fn expected_sessions(&self) -> usize {
        self.neighbor_profiles
            .iter()
            .filter(|profile| profile.is_some())
            .count()
    }
}

#[derive(Debug, Clone, JsonSerialize)]
struct CommunityDefaults {
    drain: String,
    fail_open: String,
    blackhole: String,
}

impl CommunityDefaults {
    fn recommended() -> Self {
        Self {
            drain: "no-export".to_string(),
            fail_open: "graceful-shutdown".to_string(),
            blackhole: "no-advertise".to_string(),
        }
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

fn render_bgp_alert_rules(pop: &PopDescriptor) -> String {
    let mut buf = String::new();
    let slug = sanitize_label(&pop.pop_name);
    let bfd_plan = pop.bfd_plan();
    let expected_bfd = bfd_plan.expected_sessions();
    let neighbor_regex = pop
        .neighbors
        .iter()
        .map(|n| n.address.as_str())
        .collect::<Vec<_>>()
        .join("|");

    writeln!(buf, "groups:").unwrap();
    writeln!(buf, "- name: soranet-bgp-{slug}").unwrap();
    writeln!(buf, "  rules:").unwrap();
    writeln!(buf, "  - alert: SoranetBGPNeighborDown").unwrap();
    writeln!(
        buf,
        "    expr: frr_bgp_neighbor_state{{pop=\"{}\",neighbor=~\"{}\"}} != 6",
        pop.pop_name, neighbor_regex
    )
    .unwrap();
    writeln!(buf, "    for: 5m").unwrap();
    writeln!(buf, "    labels:").unwrap();
    writeln!(buf, "      severity: page").unwrap();
    writeln!(buf, "      pop: \"{}\"", pop.pop_name).unwrap();
    writeln!(buf, "    annotations:").unwrap();
    writeln!(
        buf,
        "      summary: \"SoraNet BGP neighbor down ({})\"",
        pop.pop_name
    )
    .unwrap();
    writeln!(
        buf,
        "      description: \"One or more neighbors are not Established (expect {}).\"",
        pop.neighbors.len()
    )
    .unwrap();

    if expected_bfd > 0 {
        writeln!(buf, "  - alert: SoranetBfdSessionsDown").unwrap();
        writeln!(
            buf,
            "    expr: frr_bfd_sessions_up{{pop=\"{}\"}} < {}",
            pop.pop_name, expected_bfd
        )
        .unwrap();
        writeln!(buf, "    for: 2m").unwrap();
        writeln!(buf, "    labels:").unwrap();
        writeln!(buf, "      severity: warn").unwrap();
        writeln!(buf, "      pop: \"{}\"", pop.pop_name).unwrap();
        writeln!(buf, "    annotations:").unwrap();
        writeln!(
            buf,
            "      summary: \"BFD sessions below baseline ({})\"",
            pop.pop_name
        )
        .unwrap();
        writeln!(
            buf,
            "      description: \"BFD up sessions fell below neighbor count baseline; investigate peer/LAG health.\""
        )
        .unwrap();
    }

    if pop.rpki_caches.is_empty() {
        writeln!(
            buf,
            "  - alert: SoranetRPKIUnset\n    expr: absent(frr_rpki_cache_state{{pop=\"{}\"}})\n    for: 2m\n    labels:\n      severity: warn\n      pop: \"{}\"\n    annotations:\n      summary: \"RPKI cache not configured ({})\"\n      description: \"No RPKI cache metrics reported; ensure rpki cache <host> <port> and logging are configured.\"",
            pop.pop_name, pop.pop_name, pop.pop_name
        )
        .unwrap();
    } else {
        writeln!(buf, "  - alert: SoranetRPKIUnavailable").unwrap();
        writeln!(
            buf,
            "    expr: frr_rpki_cache_state{{pop=\"{}\"}} == 0",
            pop.pop_name
        )
        .unwrap();
        writeln!(buf, "    for: 2m").unwrap();
        writeln!(buf, "    labels:").unwrap();
        writeln!(buf, "      severity: page").unwrap();
        writeln!(buf, "      pop: \"{}\"", pop.pop_name).unwrap();
        writeln!(buf, "    annotations:").unwrap();
        writeln!(
            buf,
            "      summary: \"RPKI cache unavailable ({})\"",
            pop.pop_name
        )
        .unwrap();
        writeln!(
            buf,
            "      description: \"RPKI cache session dropped; expected {} cache(s).\"",
            pop.rpki_caches.len()
        )
        .unwrap();
    }

    let expected_prefixes = pop.prefixes.all().len();
    if expected_prefixes > 0 {
        writeln!(buf, "  - alert: SoranetPrefixDrop").unwrap();
        writeln!(
            buf,
            "    expr: frr_bgp_prefixes_received_total{{pop=\"{}\"}} < {}",
            pop.pop_name, expected_prefixes
        )
        .unwrap();
        writeln!(buf, "    for: 3m").unwrap();
        writeln!(buf, "    labels:").unwrap();
        writeln!(buf, "      severity: warn").unwrap();
        writeln!(buf, "      pop: \"{}\"", pop.pop_name).unwrap();
        writeln!(buf, "    annotations:").unwrap();
        writeln!(
            buf,
            "      summary: \"Prefix drop detected ({})\"",
            pop.pop_name
        )
        .unwrap();
        writeln!(
            buf,
            "      description: \"Received prefixes fell below the descriptor baseline ({}).\"",
            expected_prefixes
        )
        .unwrap();
    }

    for neighbor in &pop.neighbors {
        let neighbor_label = neighbor.address.as_str();
        let neighbor_slug = sanitize_label(&neighbor.name);
        if let Some(local_pref) = neighbor.local_pref {
            writeln!(buf, "  - alert: SoranetLocalPrefDrift{}", neighbor_slug).unwrap();
            writeln!(
                buf,
                "    expr: frr_bgp_prefix_bestpath_localpref{{pop=\"{}\",neighbor=~\"{}\"}} != {}",
                pop.pop_name, neighbor_label, local_pref
            )
            .unwrap();
            writeln!(buf, "    for: 3m").unwrap();
            writeln!(buf, "    labels:").unwrap();
            writeln!(buf, "      severity: warn").unwrap();
            writeln!(buf, "      pop: \"{}\"", pop.pop_name).unwrap();
            writeln!(buf, "    annotations:").unwrap();
            writeln!(
                buf,
                "      summary: \"Local-pref drift on {} ({})\"",
                neighbor.name, pop.pop_name
            )
            .unwrap();
            writeln!(
                buf,
                "      description: \"Observed local-pref no longer matches descriptor target ({}).\"",
                local_pref
            )
            .unwrap();
        }

        if let Some(med) = neighbor.med {
            writeln!(buf, "  - alert: SoranetMedDrift{}", neighbor_slug).unwrap();
            writeln!(
                buf,
                "    expr: frr_bgp_prefix_bestpath_med{{pop=\"{}\",neighbor=~\"{}\"}} != {}",
                pop.pop_name, neighbor_label, med
            )
            .unwrap();
            writeln!(buf, "    for: 3m").unwrap();
            writeln!(buf, "    labels:").unwrap();
            writeln!(buf, "      severity: warn").unwrap();
            writeln!(buf, "      pop: \"{}\"", pop.pop_name).unwrap();
            writeln!(buf, "    annotations:").unwrap();
            writeln!(
                buf,
                "      summary: \"MED drift on {} ({})\"",
                neighbor.name, pop.pop_name
            )
            .unwrap();
            writeln!(
                buf,
                "      description: \"Observed MED diverged from descriptor target ({}).\"",
                med
            )
            .unwrap();
        }
    }

    buf
}

fn render_bgp_grafana_dashboard(pop: &PopDescriptor) -> Value {
    let slug = sanitize_label(&pop.pop_name);
    let prom_uid = "${DS_PROMETHEUS}";
    let url_slug = format!("soranet-bgp-{}", slug);
    let datasource = |uid: &str| -> Value {
        let mut ds = Map::new();
        ds.insert("type".into(), Value::String("prometheus".into()));
        ds.insert("uid".into(), Value::String(uid.to_string()));
        Value::Object(ds)
    };

    let neighbor_target = Value::Object(Map::from_iter([
        (
            "expr".into(),
            Value::String(format!(
                "frr_bgp_neighbor_state{{pop=\"{}\",neighbor=~\"$neighbor\"}}",
                pop.pop_name
            )),
        ),
        ("legendFormat".into(), Value::String("{neighbor}".into())),
    ]));

    let rpki_target = Value::Object(Map::from_iter([
        (
            "expr".into(),
            Value::String(format!("frr_rpki_cache_state{{pop=\"{}\"}}", pop.pop_name)),
        ),
        ("legendFormat".into(), Value::String("{rpki_cache}".into())),
    ]));

    let prefix_target = Value::Object(Map::from_iter([
        (
            "expr".into(),
            Value::String(format!(
                "frr_bgp_prefixes_received_total{{pop=\"{}\"}}",
                pop.pop_name
            )),
        ),
        ("legendFormat".into(), Value::String("{neighbor}".into())),
    ]));

    let bfd_target = Value::Object(Map::from_iter([
        (
            "expr".into(),
            Value::String(format!("frr_bfd_sessions_up{{pop=\"{}\"}}", pop.pop_name)),
        ),
        ("legendFormat".into(), Value::String("BFD up".into())),
    ]));

    let local_pref_target = Value::Object(Map::from_iter([
        (
            "expr".into(),
            Value::String(format!(
                "frr_bgp_prefix_bestpath_localpref{{pop=\"{}\",neighbor=~\"$neighbor\"}}",
                pop.pop_name
            )),
        ),
        ("legendFormat".into(), Value::String("{neighbor}".into())),
    ]));

    let med_target = Value::Object(Map::from_iter([
        (
            "expr".into(),
            Value::String(format!(
                "frr_bgp_prefix_bestpath_med{{pop=\"{}\",neighbor=~\"$neighbor\"}}",
                pop.pop_name
            )),
        ),
        ("legendFormat".into(), Value::String("{neighbor}".into())),
    ]));

    let panel = |id: u64, title: &str, target: Value| -> Value {
        let mut map = Map::new();
        map.insert("id".into(), Value::from(id));
        map.insert("title".into(), Value::String(title.to_string()));
        map.insert("type".into(), Value::String("timeseries".into()));
        map.insert("datasource".into(), datasource(prom_uid));
        map.insert("targets".into(), Value::Array(vec![target]));
        Value::Object(map)
    };

    let mut templating_entry = Map::new();
    templating_entry.insert("name".into(), Value::String("neighbor".into()));
    templating_entry.insert("label".into(), Value::String("Neighbor".into()));
    templating_entry.insert("type".into(), Value::String("query".into()));
    templating_entry.insert("datasource".into(), datasource(prom_uid));
    templating_entry.insert(
        "query".into(),
        Value::String(format!(
            "label_values(frr_bgp_neighbor_state{{pop=\"{}\"}},neighbor)",
            pop.pop_name
        )),
    );
    templating_entry.insert("refresh".into(), Value::from(1u64));

    let mut templating = Map::new();
    templating.insert(
        "list".into(),
        Value::Array(vec![Value::Object(templating_entry)]),
    );

    let mut time = Map::new();
    time.insert("from".into(), Value::String("now-6h".into()));
    time.insert("to".into(), Value::String("now".into()));

    let mut root = Map::new();
    root.insert("uid".into(), Value::String(url_slug));
    root.insert(
        "title".into(),
        Value::String(format!("SoraNet BGP - {}", pop.pop_name)),
    );
    root.insert("schemaVersion".into(), Value::from(39u64));
    root.insert(
        "tags".into(),
        Value::Array(vec![
            Value::String("soranet".into()),
            Value::String("bgp".into()),
            Value::String(pop.pop_name.clone()),
        ]),
    );
    root.insert("time".into(), Value::Object(time));
    root.insert("templating".into(), Value::Object(templating));
    root.insert(
        "panels".into(),
        Value::Array(vec![
            panel(1, "Neighbor state", neighbor_target),
            panel(2, "BFD sessions up", bfd_target),
            panel(3, "RPKI cache health", rpki_target),
            panel(4, "Prefixes received", prefix_target),
            panel(5, "Local preference (bestpath)", local_pref_target),
            panel(6, "MED (bestpath)", med_target),
        ]),
    );

    Value::Object(root)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .wrap_err_with(|| format!("failed to open {} for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 4096];
    loop {
        let read = file
            .read(&mut buf)
            .wrap_err_with(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex_encode(hasher.finalize()))
}

fn write_pxe_profile_stub(pop_name: &str, root: &Path) -> Result<PathBuf> {
    let path = root.join("pxe_profile.toml");
    let contents = format!(
        "# Stub PXE profile for {pop}\nprofile = \"golden\"\npop = \"{pop}\"\nboot_image = \"http://pxe.invalid/soranet/{pop}/netboot.img\"\nkargs = \"console=ttyS0,115200n8\"\nnotes = \"Replace boot_image/checksums with the staged artefacts before imaging.\"\n",
        pop = pop_name
    );
    fs::write(&path, contents)
        .wrap_err_with(|| format!("failed to write PXE profile stub {}", path.display()))?;
    Ok(path)
}

fn write_health_probe_stub(pop: &PopDescriptor, root: &Path) -> Result<PathBuf> {
    let path = root.join("health_probe.sh");
    let neighbor_tokens = pop
        .neighbors
        .iter()
        .map(|n| format!("\"{}\"", n.address))
        .collect::<Vec<_>>()
        .join(" ");
    let expected_neighbors = pop.neighbors.len();
    let expected_rpki = pop.rpki_caches.len();
    let expected_prefixes = pop.prefixes.all().len();
    let contents = format!(
        "#!/usr/bin/env bash\n\
set -euo pipefail\n\
# Smoke probe for {pop} PoP after rollout. Validates BGP sessions, RPKI caches,\n\
# and received prefix counts against the PoP descriptor.\n\
if ! command -v vtysh >/dev/null 2>&1; then\n\
  echo \"vtysh missing\" >&2\n\
  exit 1\n\
fi\n\
neighbors=({neighbors})\n\
expected_neighbors={expected_neighbors}\n\
expected_rpki={expected_rpki}\n\
expected_prefixes={expected_prefixes}\n\
summary_json=\"\"\n\
if command -v jq >/dev/null 2>&1; then\n\
  summary_json=$(vtysh -c \"show bgp summary json\")\n\
fi\n\
\n\
if command -v jq >/dev/null 2>&1; then\n\
  failing=0\n\
  for n in \"${{neighbors[@]}}\"; do\n\
    state=$(echo \"$summary_json\" | jq -r --arg n \"$n\" '.ipv4Unicast.peers[$n].state // .ipv6Unicast.peers[$n].state // empty')\n\
    if [ \"$state\" != \"Established\" ]; then\n\
      echo \"neighbor $n not Established (state=$state)\" >&2\n\
      failing=1\n\
    fi\n\
  done\n\
  if [ \"$failing\" -eq 1 ]; then\n\
    exit 1\n\
  fi\n\
else\n\
  established=$(vtysh -c \"show bgp summary\" | grep -c Established || true)\n\
  if [ \"$established\" -lt \"$expected_neighbors\" ]; then\n\
    echo \"expected $expected_neighbors Established sessions, saw $established\" >&2\n\
    exit 1\n\
  fi\n\
fi\n\
\n\
if [ \"$expected_rpki\" -gt 0 ]; then\n\
  if command -v jq >/dev/null 2>&1; then\n\
    rpki_json=$(vtysh -c \"show rpki cache json\" || true)\n\
    ready=$(echo \"$rpki_json\" | jq '[.cache[] | select(.state == \"Established\")] | length' 2>/dev/null || echo 0)\n\
  else\n\
    ready=$(vtysh -c \"show rpki cache\" | grep -c Established || true)\n\
  fi\n\
  if [ \"$ready\" -lt \"$expected_rpki\" ]; then\n\
    echo \"rpki cache unhealthy (expected $expected_rpki, saw $ready)\" >&2\n\
    exit 1\n\
  fi\n\
fi\n\
\n\
if command -v jq >/dev/null 2>&1 && [ \"$expected_prefixes\" -gt 0 ]; then\n\
  prefixes=$(echo \"$summary_json\" | jq -r '((.ipv4Unicast.prefixReceivedCount // 0) + (.ipv6Unicast.prefixReceivedCount // 0))' 2>/dev/null || echo \"\")\n\
  if [ -n \"$prefixes\" ] && [ \"$prefixes\" -lt \"$expected_prefixes\" ]; then\n\
    echo \"prefix count below baseline (expected $expected_prefixes, saw $prefixes)\" >&2\n\
    exit 1\n\
  fi\n\
fi\n\
\n\
echo \"ok\"\n",
        pop = pop.pop_name,
        neighbors = neighbor_tokens,
        expected_neighbors = expected_neighbors,
        expected_rpki = expected_rpki,
        expected_prefixes = expected_prefixes
    );
    fs::write(&path, contents)
        .wrap_err_with(|| format!("failed to write health probe stub {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .wrap_err_with(|| format!("failed to read permissions for {}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).wrap_err_with(|| {
            format!(
                "failed to set executable bit on health probe {}",
                path.display()
            )
        })?;
    }
    Ok(path)
}

fn write_pop_env_template(pop: &PopDescriptor, root: &Path, image_tag: &str) -> Result<PathBuf> {
    let bfd_plan = pop.bfd_plan();
    let path = root.join("pop.env");
    let mut contents = String::new();
    writeln!(contents, "# Environment template for {} PoP", pop.pop_name).unwrap();
    writeln!(contents, "POP_NAME={}", pop.pop_name).unwrap();
    writeln!(contents, "ASN={}", pop.asn).unwrap();
    writeln!(contents, "ROUTER_ID={}", pop.router_id).unwrap();
    writeln!(contents, "LOOPBACK_IPV4={}", pop.loopback_ipv4).unwrap();
    if let Some(loopback_v6) = &pop.loopback_ipv6 {
        writeln!(contents, "LOOPBACK_IPV6={loopback_v6}").unwrap();
    }
    writeln!(contents, "IMAGE_TAG={image_tag}").unwrap();
    if pop.rpki_caches.is_empty() {
        writeln!(contents, "RPKI_CACHE_01=unset").unwrap();
    } else {
        for (idx, cache) in pop.rpki_caches.iter().enumerate() {
            writeln!(
                contents,
                "RPKI_CACHE_{:02}={}:{}",
                idx + 1,
                cache.address,
                cache.port
            )
            .unwrap();
        }
    }
    if bfd_plan.profiles.is_empty() {
        let rec = recommended_bfd_profile();
        writeln!(
            contents,
            "BFD_PROFILE_RECOMMENDED={}ms/{}ms/{}",
            rec.transmit_ms, rec.receive_ms, rec.detect_multiplier
        )
        .unwrap();
    } else {
        for profile in bfd_plan.profiles.values() {
            writeln!(
                contents,
                "BFD_PROFILE_{}={}ms/{}ms/{}",
                sanitize_label(&profile.name).to_ascii_uppercase(),
                profile.transmit_ms,
                profile.receive_ms,
                profile.detect_multiplier
            )
            .unwrap();
        }
    }
    for neighbor in &pop.neighbors {
        let label = sanitize_label(&neighbor.name)
            .to_ascii_uppercase()
            .replace('-', "_");
        writeln!(contents, "NEIGHBOR_{label}_ADDRESS={}", neighbor.address).unwrap();
        writeln!(contents, "NEIGHBOR_{label}_PEER_ASN={}", neighbor.peer_asn).unwrap();
        if let Some(ttl) = neighbor.multihop_ttl {
            writeln!(contents, "NEIGHBOR_{label}_EBGP_TTL={}", ttl).unwrap();
        }
        if neighbor.import_rpki_deny_invalid {
            writeln!(contents, "NEIGHBOR_{label}_DENY_INVALID=true").unwrap();
        }
    }
    fs::write(&path, contents)
        .wrap_err_with(|| format!("failed to write PoP env template {}", path.display()))?;
    Ok(path)
}

fn write_secrets_template(pop: &PopDescriptor, root: &Path) -> Result<PathBuf> {
    let dir = root.join("secrets");
    fs::create_dir_all(&dir)
        .wrap_err_with(|| format!("failed to create secrets directory `{}`", dir.display()))?;
    let path = dir.join("secret_placeholders.toml");
    let mut contents = String::new();
    let pop_label = sanitize_label(&pop.pop_name);
    writeln!(contents, "# Secret placeholders for {} PoP", pop.pop_name).unwrap();
    writeln!(
        contents,
        "# Replace vault:// entries prior to rollout; do not commit cleartext secrets."
    )
    .unwrap();
    writeln!(contents, "[router]").unwrap();
    writeln!(
        contents,
        "admin_password = \"vault://soranet/{}/router-admin\"",
        pop_label
    )
    .unwrap();
    writeln!(
        contents,
        "frr_api_token = \"vault://soranet/{}/frr-api-token\"",
        pop_label
    )
    .unwrap();
    writeln!(contents, "\n[rpki]").unwrap();
    writeln!(
        contents,
        "client_token = \"vault://soranet/{}/rpki-client\"",
        pop_label
    )
    .unwrap();
    if !pop.neighbors.is_empty() {
        writeln!(contents, "\n# Neighbor secrets (repeat per peer)").unwrap();
        for neighbor in &pop.neighbors {
            let key = sanitize_label(&neighbor.name);
            writeln!(contents, "[[neighbors]]").unwrap();
            writeln!(contents, "name = \"{}\"", neighbor.name).unwrap();
            writeln!(
                contents,
                "md5_password = \"vault://soranet/{}/neighbors/{}/md5_password\"",
                pop_label, key
            )
            .unwrap();
            writeln!(
                contents,
                "bfd_key = \"vault://soranet/{}/neighbors/{}/bfd_key\"",
                pop_label, key
            )
            .unwrap();
            writeln!(contents).unwrap();
        }
    }

    fs::write(&path, contents).wrap_err_with(|| {
        format!(
            "failed to write secret placeholder template {}",
            path.display()
        )
    })?;
    Ok(path)
}

fn write_bringup_checklist(pop: &PopDescriptor, root: &Path, image_tag: &str) -> Result<PathBuf> {
    let path = root.join("checklist.md");
    let mut contents = String::new();
    writeln!(contents, "# {} PoP bring-up checklist", pop.pop_name).unwrap();
    writeln!(
        contents,
        "- [ ] Stage golden image `{image_tag}` and update `pxe_profile.toml` with boot image URL + checksums."
    )
    .unwrap();
    writeln!(
        contents,
        "- [ ] Fill `pop.env` and `secrets/secret_placeholders.toml` with site defaults (vault/KMS references only)."
    )
    .unwrap();
    writeln!(
        contents,
        "- [ ] Apply `frr.conf`/`resolver.toml`; record checksums from `bundle_manifest.json` in the change packet."
    )
    .unwrap();
    writeln!(
        contents,
        "- [ ] Run `health_probe.sh` twice after peers converge and paste output into the rollout ticket."
    )
    .unwrap();
    writeln!(
        contents,
        "- [ ] Attach `attestations/signoff.json` + sigstore provenance to the change bundle and execute the CI job in `ci/pop_bringup.yml`."
    )
    .unwrap();
    writeln!(contents, "\n## Promotion gates").unwrap();
    for gate in promotion_gate_messages(&pop.pop_name, image_tag) {
        writeln!(contents, "- [ ] {gate}").unwrap();
    }
    fs::write(&path, contents)
        .wrap_err_with(|| format!("failed to write bring-up checklist {}", path.display()))?;
    Ok(path)
}

fn write_ci_stub(
    pop: &PopDescriptor,
    root: &Path,
    descriptor: &Path,
    roa_bundle: Option<&Path>,
    image_tag: &str,
) -> Result<PathBuf> {
    let dir = root.join("ci");
    fs::create_dir_all(&dir)
        .wrap_err_with(|| format!("failed to create CI directory `{}`", dir.display()))?;
    let path = dir.join("pop_bringup.yml");
    let job_label = sanitize_label(&pop.pop_name);
    let roa_flag = roa_bundle
        .map(|roa| format!(" --roa {}", roa.display()))
        .unwrap_or_default();
    let mut contents = String::new();
    writeln!(contents, "name: soranet-popctl").unwrap();
    writeln!(contents, "on:\n  workflow_dispatch:").unwrap();
    writeln!(contents, "\njobs:").unwrap();
    writeln!(contents, "  popctl-{job_label}:").unwrap();
    writeln!(contents, "    env:").unwrap();
    writeln!(contents, "      POP_NAME: {}", pop.pop_name).unwrap();
    writeln!(contents, "    runs-on: ubuntu-latest").unwrap();
    writeln!(contents, "    steps:").unwrap();
    writeln!(contents, "      - uses: actions/checkout@v4").unwrap();
    writeln!(contents, "      - name: Generate bundle").unwrap();
    writeln!(
        contents,
        "        run: |\n          cargo xtask soranet-pop-bundle --input \"{}\" --output-dir \"{}\" --image-tag \"{}\" --skip-edns --skip-ds{}\n",
        descriptor.display(),
        root.display(),
        image_tag,
        roa_flag
    )
    .unwrap();
    writeln!(contents, "      - name: Validate plan").unwrap();
    writeln!(
        contents,
        "        run: |\n          cargo xtask soranet-pop-plan --input \"{}\"{} --frr-out \"{}/frr.conf\" --json-out \"{}/plan.json\"\n",
        descriptor.display(),
        roa_flag,
        root.display(),
        root.display()
    )
    .unwrap();
    writeln!(contents, "      - name: Check attestation placeholders").unwrap();
    writeln!(
        contents,
        "        run: |\n          test -f \"{root}/bundle_manifest.json\"\n          test -f \"{root}/attestations/signoff.json\"\n",
        root = root.display()
    )
    .unwrap();

    fs::write(&path, contents)
        .wrap_err_with(|| format!("failed to write CI stub {}", path.display()))?;
    Ok(path)
}

fn write_sigstore_stub(path: &Path, pop_name: &str, image_tag: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create sigstore stub directory `{}`",
                parent.display()
            )
        })?;
    }
    let contents = format!(
        "{{\"note\":\"attach cosign/intoto provenance for {pop} image_tag={tag} here\"}}\n",
        pop = pop_name,
        tag = image_tag
    );
    fs::write(path, contents)
        .wrap_err_with(|| format!("failed to write sigstore stub {}", path.display()))?;
    Ok(())
}

fn write_signoff_bundle(path: &Path, bundle: &SignoffBundle) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create signoff parent directory `{}`",
                parent.display()
            )
        })?;
    }
    let file = File::create(path)
        .wrap_err_with(|| format!("failed to create signoff bundle {}", path.display()))?;
    json::to_writer_pretty(file, bundle)
        .wrap_err_with(|| format!("failed to serialize signoff bundle {}", path.display()))?;
    Ok(())
}

fn promotion_gate_messages(pop_name: &str, image_tag: &str) -> Vec<String> {
    let pop_slug = sanitize_label(pop_name);
    vec![
        format!(
            "Golden image `{}` signed with sigstore; `pxe_profile.toml` boot_image/checksum updated for {}",
            image_tag, pop_name
        ),
        "Secrets resolved via vault/KMS references inside secrets/secret_placeholders.toml (no cleartext)".to_string(),
        "FRR + resolver configs applied with checksums matching bundle_manifest.json".to_string(),
        "Health probe passes twice post-convergence using health_probe.sh".to_string(),
        format!(
            "Sigstore provenance recorded at attestations/soranet-{}-provenance.intoto.jsonl and attached alongside signoff.json",
            pop_slug
        ),
    ]
}

#[derive(Debug, JsonDeserialize)]
struct RoaBundle {
    #[norito(default)]
    roas: Vec<RoaRecord>,
    #[norito(skip)]
    source: Option<String>,
}

impl RoaBundle {
    fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .wrap_err_with(|| format!("failed to read ROA bundle `{}`", path.display()))?;
        let mut bundle: Self = json::from_slice(&bytes).wrap_err_with(|| {
            format!(
                "failed to parse ROA bundle `{}` as Norito JSON",
                path.display()
            )
        })?;
        bundle.validate()?;
        bundle.source = Some(path.display().to_string());
        Ok(bundle)
    }

    fn validate(&self) -> Result<()> {
        for roa in &self.roas {
            roa.validate()?;
        }
        Ok(())
    }

    fn matches(&self, prefix: &str, asn: u32) -> bool {
        self.roas
            .iter()
            .any(|entry| entry.asn == asn && entry.prefix == prefix)
    }
}

#[derive(Debug, JsonDeserialize)]
struct RoaRecord {
    prefix: String,
    max_length: u8,
    asn: u32,
}

impl RoaRecord {
    fn validate(&self) -> Result<()> {
        let (addr, prefix_len) = self
            .prefix
            .split_once('/')
            .ok_or_else(|| eyre!("ROA prefix `{}` is missing a slash", self.prefix))?;
        let _ip: IpAddr = addr
            .parse()
            .wrap_err_with(|| format!("ROA prefix `{}` has an invalid IP", self.prefix))?;
        let max_len: u8 = prefix_len
            .parse()
            .wrap_err_with(|| format!("ROA prefix `{}` has an invalid length", self.prefix))?;
        if self.max_length < max_len {
            return Err(eyre!(
                "ROA `{}` uses max_length {} shorter than prefix length {}",
                self.prefix,
                self.max_length,
                max_len
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn renders_frr_config() {
        let json = br#"{
            "pop_name":"sjc01",
            "asn":65210,
            "router_id":"10.10.10.1",
            "loopback_ipv4":"203.0.113.10",
            "loopback_ipv6":"2001:db8:10::a",
            "bfd_profiles":[{"name":"fast","transmit_ms":300,"receive_ms":300,"detect_multiplier":3}],
            "rpki_caches":[{"address":"rpki.sora.net","port":3323}],
            "prefixes":{"ipv4":["203.0.113.0/24"],"ipv6":["2001:db8:10::/48"]},
            "neighbors":[
                {"name":"ix-west","address":"198.51.100.1","peer_asn":64512,"bfd_profile":"fast","local_pref":300,"med":20,"import_rpki_deny_invalid":true},
                {"name":"core-v6","address":"2001:db8:feed::1","peer_asn":64520,"bfd_profile":"fast","multihop_ttl":2}
            ]
        }"#;
        let descriptor: PopDescriptor = json::from_slice(json).expect("parse descriptor");
        descriptor.validate().expect("valid descriptor");
        let rendered = descriptor.render_config().expect("render");
        assert!(rendered.contains("router bgp 65210"));
        assert!(rendered.contains("neighbor 198.51.100.1 route-map RM_ix-west_IMPORT in"));
        assert!(rendered.contains("match rpki invalid"));
        assert!(rendered.contains("SORANET-DRAIN permit no-export"));
        assert!(rendered.contains("SORANET-FAIL-OPEN permit graceful-shutdown"));
        assert!(rendered.contains("SORANET-BLACKHOLE permit no-advertise"));
        assert!(rendered.contains("neighbor 2001:db8:feed::1 activate"));
        assert!(rendered.contains("route-map RM_ix-west_EXPORT permit 10"));
    }

    #[test]
    fn validates_roa_and_bfd_gaps() {
        let descriptor = br#"{
            "pop_name":"sjc01",
            "asn":65210,
            "router_id":"10.10.10.1",
            "loopback_ipv4":"203.0.113.10",
            "bfd_profiles":[{"name":"fast","transmit_ms":300,"receive_ms":300,"detect_multiplier":3}],
            "prefixes":{"ipv4":["203.0.113.0/24"],"ipv6":["2001:db8:10::/48"]},
            "neighbors":[
                {"name":"ix-west","address":"198.51.100.1","peer_asn":64512,"bfd_profile":"fast","local_pref":300,"med":20,"import_rpki_deny_invalid":true},
                {"name":"core-v6","address":"2001:db8:feed::1","peer_asn":64520,"multihop_ttl":2}
            ]
        }"#;
        let roas = br#"{
            "roas":[
                {"prefix":"203.0.113.0/24","max_length":24,"asn":65210}
            ]
        }"#;

        let dir = TempDir::new().expect("temp dir");
        let descriptor_path = write_temp_file(dir.path(), "pop.json", descriptor);
        let roa_path = write_temp_file(dir.path(), "roas.json", roas);

        let report =
            validate_pop_descriptor(&descriptor_path, Some(&roa_path)).expect("validation report");
        assert_eq!(report.pop_name, "sjc01");
        assert_eq!(report.asn, 65210);
        assert_eq!(report.rpki_caches, 0);
        assert!(!report.has_rpki_cache);
        assert_eq!(
            report.rpki_cache_warnings,
            vec!["no RPKI cache configured; enable rpki cache <host> <port> and logging"]
        );
        assert_eq!(report.missing_bfd_neighbors, vec!["core-v6"]);
        assert_eq!(
            report.prefixes_without_roa,
            vec!["2001:db8:10::/48".to_string()]
        );
        assert_eq!(report.recommended_bfd_profile.detect_multiplier, 3);
        assert_eq!(report.communities.drain, "no-export");
        assert!(report.communities.using_defaults);
        assert_eq!(report.communities.blackhole, "no-advertise");
        assert_eq!(report.communities.fail_open, "graceful-shutdown");
        assert_eq!(report.bfd_autofill_neighbors, vec!["core-v6".to_string()]);
        assert_eq!(report.bfd_recommendations.len(), 1);
        let value = report.to_value();
        assert_eq!(value["pop_name"], Value::String("sjc01".to_string()));
    }

    #[test]
    fn rejects_invalid_community_strings() {
        assert!(validate_community_value("drain", "65535:notanumber").is_err());
        assert!(validate_community_value("drain", "no-export").is_ok());
        assert!(validate_community_value("drain", "65535:666").is_ok());
        assert!(validate_community_value("drain", "not a community").is_err());
    }

    fn write_temp_file(dir: &Path, name: &str, contents: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).expect("write temp file");
        path
    }
}
