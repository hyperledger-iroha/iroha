//! Compute lane SLO harness and SDK/CLI parity fixtures.

use std::{
    collections::BTreeMap,
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr};
use iroha_config::parameters::defaults::compute as compute_defaults;
use iroha_data_model::compute::{
    ComputeAuthz, ComputeCallSummary, ComputeCodec, ComputeManifest, ComputeReceipt, ComputeRouteId,
};
use norito::{
    derive::{JsonDeserialize as DeriveJsonDeserialize, JsonSerialize as DeriveJsonSerialize},
    json::{self, JsonSerialize},
};

use crate::compute_harness::{
    ComputeHarnessError, SloTargets, build_call_for_route, charge_units, default_manifest,
    execute_entrypoint, meter, payload_with_len, slo_targets,
};

/// Options for generating an SLO report.
#[derive(Debug, Clone)]
pub struct ComputeSloReportOptions {
    /// Compute manifest to exercise (defaults to fixtures).
    pub manifest: PathBuf,
    /// JSON output path.
    pub json_out: PathBuf,
    /// Optional Markdown summary output.
    pub markdown_out: Option<PathBuf>,
    /// Number of samples per route.
    pub samples: NonZeroUsize,
}

/// Options for generating cross-SDK fixtures.
#[derive(Debug, Clone)]
pub struct ComputeFixtureOptions {
    /// Destination directory for fixtures.
    pub output_dir: PathBuf,
}

/// Run a deterministic SLO check against a manifest and write JSON/Markdown reports.
pub fn run_slo_report(options: ComputeSloReportOptions) -> Result<()> {
    let manifest = load_manifest(&options.manifest)?;
    let targets = slo_targets();
    let report = build_report(&manifest, targets, options.samples)?;
    write_json(&options.json_out, &report)?;
    if let Some(md) = &options.markdown_out {
        write_markdown(md, &report)?;
    }
    Ok(())
}

/// Emit compute fixtures for SDK/CLI parity (call + receipt + rejection catalog).
pub fn write_fixtures(options: ComputeFixtureOptions) -> Result<()> {
    fs::create_dir_all(&options.output_dir)
        .wrap_err("failed to create compute fixture directory")?;
    let manifest_path = options.output_dir.join("manifest_compute_payments.json");
    let manifest = load_manifest(&manifest_path).unwrap_or_else(|_| default_manifest());
    // Happy path fixture.
    let payload = payload_with_len(48);
    let call = build_call_for_route(
        &manifest,
        &manifest.routes[0],
        &payload,
        ComputeCodec::NoritoJson,
        ComputeAuthz::Public,
    )?;
    let (outcome, response) = execute_entrypoint(&manifest.routes[0].entrypoint, &payload, &call)
        .map_err(|err| eyre::eyre!(err.to_string()))?;
    let mut metering = meter(&manifest.routes[0], &call, &payload, &response);
    charge_units(
        &compute_defaults::price_families(),
        &compute_defaults::default_price_family(),
        &compute_defaults::price_amplifiers(),
        compute_defaults::max_cu_per_call(),
        compute_defaults::max_amplification_ratio(),
        &call,
        &mut metering,
    );
    let receipt = ComputeReceipt {
        call: ComputeCallSummary::from(&call),
        metering,
        outcome,
    };

    let call_path = options.output_dir.join("call_compute_payments_parity.json");
    let receipt_path = options
        .output_dir
        .join("receipt_compute_payments_parity.json");
    write_json(&call_path, &call)?;
    write_json(&receipt_path, &receipt)?;

    // Rejection catalog is static and mirrors the gateway error mapping.
    let catalog_path = options.output_dir.join("rejection_compute_catalog.json");
    let catalog = rejection_catalog(&manifest.routes[0].id);
    write_json(&catalog_path, &catalog)?;
    Ok(())
}

fn build_report(
    manifest: &ComputeManifest,
    targets: SloTargets,
    samples: NonZeroUsize,
) -> Result<ComputeSloReport> {
    let mut routes = Vec::new();
    for route in &manifest.routes {
        routes.push(route_report(
            manifest,
            route,
            targets,
            samples.get(),
            &compute_defaults::price_families(),
            compute_defaults::default_price_family(),
        )?);
    }
    let pass = routes.iter().all(|r| r.pass);
    Ok(ComputeSloReport {
        manifest_namespace: manifest.namespace.to_string(),
        samples: samples.get(),
        targets: targets.into(),
        routes,
        pass,
    })
}

fn route_report(
    manifest: &ComputeManifest,
    route: &iroha_data_model::compute::ComputeRoute,
    targets: SloTargets,
    samples: usize,
    price_families: &BTreeMap<
        iroha_data_model::name::Name,
        iroha_data_model::compute::ComputePriceWeights,
    >,
    default_price_family: iroha_data_model::name::Name,
) -> Result<RouteReport> {
    let mut durations = Vec::with_capacity(samples);
    let mut egress_bytes = Vec::with_capacity(samples);
    let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
    let receipts = Vec::new();

    for idx in 0..samples {
        let payload = payload_with_len(sample_payload_len(idx));
        let call = build_call_for_route(
            manifest,
            route,
            &payload,
            ComputeCodec::NoritoJson,
            ComputeAuthz::Public,
        )
        .map_err(|err| eyre::eyre!(err.to_string()))?;
        let (outcome, response) =
            execute_entrypoint(&route.entrypoint, &payload, &call).map_err(|err| match err {
                ComputeHarnessError::UnknownEntrypoint(entrypoint) => eyre::eyre!(
                    "unknown entrypoint `{entrypoint}` in compute manifest route {:?}",
                    route.id
                ),
            })?;
        let mut metering = meter(route, &call, &payload, &response);
        charge_units(
            price_families,
            &default_price_family,
            &compute_defaults::price_amplifiers(),
            compute_defaults::max_cu_per_call(),
            compute_defaults::max_amplification_ratio(),
            &call,
            &mut metering,
        );

        *outcomes.entry(format!("{:?}", outcome.kind)).or_default() += 1;
        durations.push(metering.duration_ms);
        egress_bytes.push(metering.egress_bytes);
    }

    let p50 = percentile(&durations, 0.50);
    let p95 = percentile(&durations, 0.95);
    let p99 = percentile(&durations, 0.99);
    let total_duration_ms: u64 = durations.iter().copied().sum();
    let throughput_rps = if total_duration_ms == 0 {
        samples as f64
    } else {
        samples as f64 / (total_duration_ms as f64 / 1_000.0)
    };

    let pass = p50 <= targets.target_p50_latency_ms.get()
        && p95 <= targets.target_p95_latency_ms.get()
        && p99 <= targets.target_p99_latency_ms.get();

    Ok(RouteReport {
        id: route.id.clone(),
        samples,
        p50_latency_ms: p50,
        p95_latency_ms: p95,
        p99_latency_ms: p99,
        throughput_rps,
        max_egress_bytes: *egress_bytes.iter().max().unwrap_or(&0),
        response_cap_bytes: route.max_response_bytes.get(),
        outcomes,
        receipts,
        pass,
    })
}

#[derive(Debug, Clone, DeriveJsonSerialize, DeriveJsonDeserialize)]
struct ComputeSloReport {
    manifest_namespace: String,
    samples: usize,
    targets: TargetsView,
    routes: Vec<RouteReport>,
    pass: bool,
}

#[derive(Debug, Clone, DeriveJsonSerialize, DeriveJsonDeserialize)]
struct RouteReport {
    id: ComputeRouteId,
    samples: usize,
    p50_latency_ms: u64,
    p95_latency_ms: u64,
    p99_latency_ms: u64,
    throughput_rps: f64,
    max_egress_bytes: u64,
    response_cap_bytes: u64,
    outcomes: BTreeMap<String, usize>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    receipts: Vec<ComputeReceipt>,
    pass: bool,
}

#[derive(Debug, Clone, DeriveJsonSerialize, DeriveJsonDeserialize)]
struct TargetsView {
    max_inflight_per_route: usize,
    queue_depth_per_route: usize,
    max_requests_per_second: u32,
    target_p50_latency_ms: u64,
    target_p95_latency_ms: u64,
    target_p99_latency_ms: u64,
}

impl From<SloTargets> for TargetsView {
    fn from(targets: SloTargets) -> Self {
        Self {
            max_inflight_per_route: targets.max_inflight_per_route.get(),
            queue_depth_per_route: targets.queue_depth_per_route.get(),
            max_requests_per_second: targets.max_requests_per_second.get(),
            target_p50_latency_ms: targets.target_p50_latency_ms.get(),
            target_p95_latency_ms: targets.target_p95_latency_ms.get(),
            target_p99_latency_ms: targets.target_p99_latency_ms.get(),
        }
    }
}

#[derive(Debug, Clone, DeriveJsonSerialize, DeriveJsonDeserialize)]
struct RejectionCase {
    case: String,
    status: u16,
    message: String,
}

fn load_manifest(path: &Path) -> Result<ComputeManifest> {
    if path.exists() {
        let bytes =
            fs::read(path).wrap_err_with(|| format!("failed to read {}", path.display()))?;
        return json::from_slice(&bytes).wrap_err("failed to decode compute manifest");
    }
    Ok(default_manifest())
}

fn write_json<T: JsonSerialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!("failed to create parent directory for {}", path.display())
        })?;
    }
    let json = json::to_json_pretty(value)?;
    fs::write(path, json).wrap_err_with(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn write_markdown(path: &Path, report: &ComputeSloReport) -> Result<()> {
    let mut md = String::new();
    md.push_str("# Compute SLO Report\n\n");
    md.push_str(&format!(
        "- Namespace: `{}`\n- Samples per route: {}\n- Targets: p50 {}ms, p95 {}ms, p99 {}ms; max RPS {}; in-flight {}; queue {}\n\n",
        report.manifest_namespace,
        report.samples,
        report.targets.target_p50_latency_ms,
        report.targets.target_p95_latency_ms,
        report.targets.target_p99_latency_ms,
        report.targets.max_requests_per_second,
        report.targets.max_inflight_per_route,
        report.targets.queue_depth_per_route,
    ));
    md.push_str("| Route | p50 (ms) | p95 (ms) | p99 (ms) | Throughput (req/s est.) | Max egress (bytes) | Cap (bytes) | Pass |\n");
    md.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for route in &report.routes {
        md.push_str(&format!(
            "| `{:?}` | {} | {} | {} | {:.2} | {} | {} | {} |\n",
            route.id,
            route.p50_latency_ms,
            route.p95_latency_ms,
            route.p99_latency_ms,
            route.throughput_rps,
            route.max_egress_bytes,
            route.response_cap_bytes,
            if route.pass { "✅" } else { "❌" }
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, md).wrap_err_with(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn percentile(values: &[u64], quantile: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let len = sorted.len();
    let idx = (((len as f64 - 1.0) * quantile).ceil() as usize).min(len - 1);
    sorted[idx]
}

fn sample_payload_len(idx: usize) -> usize {
    const PAYLOAD_LADDER: [usize; 6] = [0, 8, 64, 256, 512, 1024];
    PAYLOAD_LADDER[idx % PAYLOAD_LADDER.len()]
}

fn rejection_catalog(route: &ComputeRouteId) -> Vec<RejectionCase> {
    vec![
        RejectionCase {
            case: "payload_too_large".to_string(),
            status: 413,
            message: "payload exceeds route cap".to_string(),
        },
        RejectionCase {
            case: "replay".to_string(),
            status: 409,
            message: "duplicate compute request (replay detected)".to_string(),
        },
        RejectionCase {
            case: "rate_limited".to_string(),
            status: 429,
            message: "compute rate limit exceeded".to_string(),
        },
        RejectionCase {
            case: "queue_full".to_string(),
            status: 503,
            message: "compute queue saturated".to_string(),
        },
        RejectionCase {
            case: "unknown_route".to_string(),
            status: 404,
            message: format!("route {route:?} not found in manifest"),
        },
    ]
}

#[cfg(test)]
mod tests {
    use iroha_data_model::compute::ComputeCall;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn percentile_handles_small_sets() {
        assert_eq!(percentile(&[], 0.5), 0);
        assert_eq!(percentile(&[10], 0.5), 10);
        assert_eq!(percentile(&[10, 20, 30, 40], 0.75), 40);
    }

    #[test]
    fn slo_report_writes_outputs() {
        let tmp = TempDir::new().expect("tempdir");
        let json_out = tmp.path().join("slo.json");
        let md_out = tmp.path().join("slo.md");
        let opts = ComputeSloReportOptions {
            manifest: PathBuf::from("fixtures/compute/manifest_compute_payments.json"),
            json_out: json_out.clone(),
            markdown_out: Some(md_out.clone()),
            samples: NonZeroUsize::new(8).unwrap(),
        };
        run_slo_report(opts).expect("slo report");
        assert!(json_out.exists());
        assert!(md_out.exists());
        let parsed: ComputeSloReport =
            json::from_slice(&fs::read(json_out).expect("read")).expect("decode report");
        assert!(!parsed.routes.is_empty());
        assert!(parsed.pass);
    }

    #[test]
    fn fixtures_are_emitted() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = ComputeFixtureOptions {
            output_dir: tmp.path().to_path_buf(),
        };
        write_fixtures(opts).expect("fixtures");
        let call_path = tmp.path().join("call_compute_payments_parity.json");
        let receipt_path = tmp.path().join("receipt_compute_payments_parity.json");
        assert!(call_path.exists());
        assert!(receipt_path.exists());
        let call: ComputeCall =
            json::from_slice(&fs::read(call_path).expect("read")).expect("call");
        let receipt: ComputeReceipt =
            json::from_slice(&fs::read(receipt_path).expect("read")).expect("receipt");
        assert_eq!(call.route.service.to_string(), "payments");
        assert_eq!(receipt.call.route.method.to_string(), "quote");
    }
}
