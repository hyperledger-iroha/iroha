//! Compute lane helpers for SDK/CLI parity.
use crate::{Run, RunContext};
use base64::Engine as _;
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::compute::{
    ComputeAuthPolicy, ComputeAuthz, ComputeCall, ComputeCallSummary, ComputeCodec,
    ComputeManifest, ComputeMetering, ComputeOutcome, ComputeOutcomeKind, ComputePriceWeights,
    ComputeRoute, ComputeRouteId, ComputeSandboxRules,
};
use iroha_config::parameters::defaults::compute as compute_defaults;
use iroha_crypto::Hash;
use norito::derive::{JsonDeserialize, JsonSerialize};
use norito::json;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
const DEFAULT_MANIFEST: &str = "fixtures/compute/manifest_compute_payments.json";
const DEFAULT_CALL: &str = "fixtures/compute/call_compute_payments.json";
const DEFAULT_PAYLOAD: &str = "fixtures/compute/payload_compute_payments.json";
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8088";
// Compute descriptors are control-plane JSON rather than bulk payloads. One MiB
// admits the canonical fixtures and a large route/header catalogue while keeping
// parser graph growth independent from an arbitrary local file.
const COMPUTE_DESCRIPTOR_MAX_BYTES_V1: usize = 1024 * 1024;
const COMPUTE_PAYLOAD_MAX_BYTES_V1: usize = super::MAX_CLI_STDIN_BYTES_V1;
const COMPUTE_JSON_MAX_SEQUENCE_ELEMENTS_V1: usize = 65_536;
const COMPUTE_JSON_MAX_TOTAL_ELEMENTS_V1: usize = 4 * COMPUTE_JSON_MAX_SEQUENCE_ELEMENTS_V1;
const COMPUTE_JSON_MAX_DECODE_ALLOCATION_BYTES_V1: usize = 16 * 1024 * 1024;
const COMPUTE_JSON_MAX_NESTING_DEPTH_V1: usize = 32;
const COMPUTE_RESPONSE_FIXED_OVERHEAD_BYTES_V1: usize = COMPUTE_DESCRIPTOR_MAX_BYTES_V1;
const COMPUTE_DESCRIPTOR_DECODE_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    COMPUTE_JSON_MAX_SEQUENCE_ELEMENTS_V1,
    COMPUTE_DESCRIPTOR_MAX_BYTES_V1,
    COMPUTE_JSON_MAX_TOTAL_ELEMENTS_V1,
    COMPUTE_JSON_MAX_DECODE_ALLOCATION_BYTES_V1,
    COMPUTE_JSON_MAX_NESTING_DEPTH_V1,
);
/// Compute CLI commands.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Simulate a compute call offline and emit the receipt/response.
    Simulate(SimulateArgs),
    /// Invoke a running compute gateway using the shared fixtures.
    Invoke(InvokeArgs),
}
/// Arguments for compute simulation.
#[derive(clap::Args, Debug)]
pub struct SimulateArgs {
    /// Path to the compute manifest to validate against.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// Path to the canonical compute call fixture.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CALL)]
    call: PathBuf,
    /// Path to the payload to send (ignored when --payload-inline is supplied).
    #[arg(long, value_name = "PATH", default_value = DEFAULT_PAYLOAD)]
    payload: PathBuf,
    /// Inline payload bytes (UTF-8) (mutually exclusive with --payload).
    #[arg(long, value_name = "BYTES")]
    payload_inline: Option<String>,
    /// Optional JSON output path (stdout when omitted).
    #[arg(long, value_name = "PATH")]
    json_out: Option<PathBuf>,
}
/// Arguments for invoking a running compute gateway.
#[derive(clap::Args, Debug)]
pub struct InvokeArgs {
    /// Base endpoint for the compute gateway (without the route path).
    #[arg(long, value_name = "URL", default_value = DEFAULT_ENDPOINT)]
    endpoint: String,
    /// Path to the compute manifest used for validation.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// Path to the compute call fixture.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CALL)]
    call: PathBuf,
    /// Path to the payload to send with the call.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_PAYLOAD)]
    payload: PathBuf,
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Simulate(args) => {
                let output_path = args.json_out.clone();
                let output = simulate(&args)?;
                if let Some(path) = output_path {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)
                            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
                    }
                    let json = json::to_vec_pretty(&output)?;
                    fs::write(&path, json)
                        .wrap_err_with(|| format!("failed to write {}", path.display()))?;
                    context.println(format!("wrote compute receipt to {}", path.display()))?;
                } else {
                    context.print_data(&output)?;
                }
                Ok(())
            }
            Command::Invoke(args) => {
                let output = invoke(&args)?;
                context.print_data(&output)?;
                Ok(())
            }
        }
    }
}
/// Output of the compute simulate/invoke commands.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SimulateOutput {
    /// Canonical call built from the manifest/fixtures.
    call: ComputeCall,
    /// Receipt emitted by the harness.
    receipt: iroha::data_model::compute::ComputeReceipt,
    /// Optional base64-encoded response payload.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    response_b64: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct GatewayRequest {
    namespace: iroha::data_model::name::Name,
    codec: ComputeCodec,
    ttl_slots: std::num::NonZeroU64,
    gas_limit: std::num::NonZeroU64,
    max_response_bytes: std::num::NonZeroU64,
    determinism: iroha::data_model::compute::ComputeDeterminism,
    execution_class: iroha::data_model::compute::ComputeExecutionClass,
    #[norito(default)]
    declared_input_bytes: Option<std::num::NonZeroU64>,
    #[norito(default)]
    declared_input_chunks: Option<std::num::NonZeroU32>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    sponsor_budget_cu: Option<std::num::NonZeroU64>,
    price_family: iroha::data_model::name::Name,
    resource_profile: iroha::data_model::name::Name,
    auth: ComputeAuthz,
    #[norito(default)]
    headers: BTreeMap<String, String>,
    /// Base64-encoded payload bytes.
    payload_b64: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct GatewayResponse {
    receipt: iroha::data_model::compute::ComputeReceipt,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    response_b64: Option<String>,
}
fn simulate(args: &SimulateArgs) -> Result<SimulateOutput> {
    let manifest = load_manifest(&args.manifest)?;
    let call = load_call(&args.call)?;
    let payload_limit = compute_payload_limit(&manifest, &call)?;
    let payload = load_payload(&args.payload, args.payload_inline.as_ref(), payload_limit)?;
    simulate_with_call(&manifest, call, &payload)
}
fn invoke(args: &InvokeArgs) -> Result<SimulateOutput> {
    invoke_with_sender(args, |url, body, max_response_body_bytes| {
        let client = reqwest::blocking::Client::new();
        let mut resp = client
            .post(&url)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .wrap_err("failed to send compute request")?;
        let status = resp.status();
        if !status.is_success() {
            return Err(eyre!("compute gateway returned status {status}"));
        }
        let declared_bytes = resp.content_length();
        read_compute_response_body_bounded(&mut resp, declared_bytes, max_response_body_bytes)
    })
}
fn invoke_with_sender<F>(args: &InvokeArgs, sender: F) -> Result<SimulateOutput>
where
    F: FnOnce(String, Vec<u8>, usize) -> Result<Vec<u8>>,
{
    let manifest = load_manifest(&args.manifest)?;
    let call = load_call(&args.call)?;
    let route = call.route.clone();
    let payload_limit = compute_payload_limit(&manifest, &call)?;
    let payload = load_payload(&args.payload, None, payload_limit)?;
    ensure_payload_hash(&call, payload.as_slice())?;
    let request = build_gateway_request(&call, &payload)?;
    let max_response_body_bytes = gateway_response_body_limit(call.max_response_bytes.get())?;
    let request_body = encode_gateway_request_bounded(&request, payload.len())?;
    drop(request);
    drop(payload);
    let url = format!(
        "{}/v1/compute/{}/{}",
        args.endpoint.trim_end_matches('/'),
        route.service,
        route.method
    );
    let response_bytes = sender(url, request_body, max_response_body_bytes)?;
    let parsed = decode_gateway_response_bounded(
        &response_bytes,
        call.max_response_bytes.get(),
        max_response_body_bytes,
    )?;
    drop(response_bytes);
    Ok(SimulateOutput {
        call,
        receipt: parsed.receipt,
        response_b64: parsed.response_b64,
    })
}
fn simulate_with_call(
    manifest: &ComputeManifest,
    call: ComputeCall,
    payload: &[u8],
) -> Result<SimulateOutput> {
    manifest.validate()?;
    manifest.validate_call(&call)?;
    let route = manifest
        .route(&call.route)
        .ok_or_else(|| eyre!("route {:?} missing from manifest", call.route))?;
    if payload.len() as u64 > route.max_request_bytes.get() {
        return Err(eyre!(
            "payload size {} exceeds max_request_bytes {}",
            payload.len(),
            route.max_request_bytes
        ));
    }
    ensure_payload_hash(&call, payload)?;
    let (outcome, response) = execute_entrypoint(&route.entrypoint, payload, &call)?;
    let mut metering = meter(route, &call, payload, response.as_ref());
    let price_families = compute_defaults::price_families();
    charge_units(
        &price_families,
        &compute_defaults::default_price_family(),
        &mut metering,
    );
    let receipt = iroha::data_model::compute::ComputeReceipt {
        call: ComputeCallSummary::from(&call),
        metering,
        outcome,
    };
    let response_b64 = response
        .as_ref()
        .map(|resp| encode_base64_bounded(resp))
        .transpose()?;
    Ok(SimulateOutput {
        call,
        receipt,
        response_b64,
    })
}
fn load_manifest(path: &Path) -> Result<ComputeManifest> {
    if !path.exists() {
        return Ok(default_manifest());
    }
    decode_compute_json_file(path, "compute manifest")
}
fn load_call(path: &Path) -> Result<ComputeCall> {
    decode_compute_json_file(path, "compute call")
}
fn load_payload(path: &Path, inline: Option<&String>, max_bytes: usize) -> Result<Vec<u8>> {
    if let Some(bytes) = inline {
        if bytes.len() > max_bytes {
            return Err(eyre!(
                "payload size {} exceeds max_request_bytes/CLI limit {max_bytes}",
                bytes.len()
            ));
        }
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(bytes.len())
            .map_err(|error| eyre!("failed to reserve inline compute payload: {error}"))?;
        payload.extend_from_slice(bytes.as_bytes());
        return Ok(payload);
    }
    let selected = if path.exists() || path == Path::new(DEFAULT_PAYLOAD) {
        path
    } else {
        Path::new(DEFAULT_PAYLOAD)
    };
    read_compute_file_bounded(selected, max_bytes, "compute payload")
        .wrap_err_with(|| format!("failed to read payload from {}", path.display()))
}
fn compute_payload_limit(manifest: &ComputeManifest, call: &ComputeCall) -> Result<usize> {
    manifest.validate()?;
    manifest.validate_call(call)?;
    let route = manifest
        .route(&call.route)
        .ok_or_else(|| eyre!("route {:?} missing from manifest", call.route))?;
    Ok(usize::try_from(route.max_request_bytes.get())
        .unwrap_or(usize::MAX)
        .min(COMPUTE_PAYLOAD_MAX_BYTES_V1))
}
fn read_compute_file_bounded(path: &Path, max_bytes: usize, label: &str) -> Result<Vec<u8>> {
    let path_metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} {}", path.display()))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(eyre!(
            "{label} must be a direct regular file: {}",
            path.display()
        ));
    }
    if path_metadata.len() > max_bytes as u64 {
        return Err(eyre!(
            "{label} {} exceeds the first-release limit of {max_bytes} bytes",
            path.display()
        ));
    }
    let mut file = fs::File::open(path)
        .wrap_err_with(|| format!("failed to open {label} {}", path.display()))?;
    let before = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {label} {}", path.display()))?;
    if !before.is_file() || before.len() > max_bytes as u64 {
        return Err(eyre!(
            "{label} must be a regular file within {max_bytes} bytes: {}",
            path.display()
        ));
    }
    let bytes = super::read_cli_input_bounded(&mut file, max_bytes, label)
        .wrap_err_with(|| format!("failed to read {label} {}", path.display()))?;
    let after = file
        .metadata()
        .wrap_err_with(|| format!("failed to reinspect {label} {}", path.display()))?;
    if before.len() != after.len() || after.len() != bytes.len() as u64 {
        return Err(eyre!("{label} changed while reading: {}", path.display()));
    }
    Ok(bytes)
}
fn decode_compute_json_file<T>(path: &Path, label: &str) -> Result<T>
where
    T: norito::json::JsonDeserialize,
{
    let bytes = read_compute_file_bounded(path, COMPUTE_DESCRIPTOR_MAX_BYTES_V1, label)?;
    preflight_compute_json(&bytes, COMPUTE_DESCRIPTOR_DECODE_LIMITS_V1, label)?;
    norito::with_decode_limits_scope(COMPUTE_DESCRIPTOR_DECODE_LIMITS_V1, || {
        json::from_slice(&bytes)
    })
    .wrap_err_with(|| format!("failed to decode {label} {}", path.display()))
}
fn preflight_compute_json(bytes: &[u8], limits: norito::DecodeLimits, label: &str) -> Result<()> {
    json::preflight_slice(
        bytes,
        json::JsonPreflightLimits::from_decode_limits(bytes.len(), limits),
    )
    .map(|_| ())
    .map_err(|error| eyre!("{label} JSON exceeds its lexical resource bounds: {error}"))
}
fn base64_encoded_len(bytes: usize) -> Result<usize> {
    base64::encoded_len(bytes, true).ok_or_else(|| eyre!("base64 length overflow"))
}
fn encode_base64_bounded(bytes: &[u8]) -> Result<String> {
    let encoded_len = base64_encoded_len(bytes.len())?;
    let mut encoded = String::new();
    encoded
        .try_reserve_exact(encoded_len)
        .map_err(|error| eyre!("failed to reserve base64 payload storage: {error}"))?;
    base64::engine::general_purpose::STANDARD.encode_string(bytes, &mut encoded);
    if encoded.len() != encoded_len {
        return Err(eyre!("base64 encoder length mismatch"));
    }
    Ok(encoded)
}
fn gateway_response_body_limit(max_response_bytes: u64) -> Result<usize> {
    let payload_bytes = usize::try_from(max_response_bytes)
        .map_err(|_| eyre!("compute response limit cannot be represented on this host"))?;
    if payload_bytes > COMPUTE_PAYLOAD_MAX_BYTES_V1 {
        return Err(eyre!(
            "compute response limit {payload_bytes} exceeds the first-release CLI limit of {} bytes",
            COMPUTE_PAYLOAD_MAX_BYTES_V1
        ));
    }
    base64_encoded_len(payload_bytes)?
        .checked_add(COMPUTE_RESPONSE_FIXED_OVERHEAD_BYTES_V1)
        .ok_or_else(|| eyre!("compute response envelope length overflow"))
}
fn gateway_request_body_limit(payload_bytes: usize) -> Result<usize> {
    base64_encoded_len(payload_bytes)?
        .checked_add(COMPUTE_DESCRIPTOR_MAX_BYTES_V1)
        .ok_or_else(|| eyre!("compute request envelope length overflow"))
}
fn encode_gateway_request_bounded(
    request: &GatewayRequest,
    payload_bytes: usize,
) -> Result<Vec<u8>> {
    let max_bytes = gateway_request_body_limit(payload_bytes)?;
    json::to_json_bounded(request, max_bytes)
        .map(String::into_bytes)
        .map_err(|error| eyre!("failed to encode bounded compute gateway request: {error}"))
}
fn read_compute_response_body_bounded<R: std::io::Read>(
    reader: &mut R,
    declared_bytes: Option<u64>,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    if declared_bytes.is_some_and(|length| length > max_bytes as u64) {
        return Err(eyre!(
            "compute response Content-Length exceeds the first-release limit of {max_bytes} bytes"
        ));
    }
    super::read_cli_input_bounded(reader, max_bytes, "compute response body")
}
fn decode_gateway_response_bounded(
    bytes: &[u8],
    max_payload_bytes: u64,
    max_body_bytes: usize,
) -> Result<GatewayResponse> {
    if bytes.len() > max_body_bytes {
        return Err(eyre!(
            "compute response body exceeds the first-release limit of {max_body_bytes} bytes"
        ));
    }
    let max_allocated_bytes = max_body_bytes
        .checked_mul(2)
        .ok_or_else(|| eyre!("compute response decode budget overflow"))?;
    let limits = norito::DecodeLimits::new(
        COMPUTE_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        max_body_bytes,
        COMPUTE_JSON_MAX_TOTAL_ELEMENTS_V1,
        max_allocated_bytes,
        COMPUTE_JSON_MAX_NESTING_DEPTH_V1,
    );
    preflight_compute_json(bytes, limits, "compute response")?;
    let response: GatewayResponse =
        norito::with_decode_limits_scope(limits, || json::from_slice(bytes))
            .wrap_err("failed to decode compute gateway response")?;
    if let Some(encoded) = &response.response_b64 {
        let payload_bytes = usize::try_from(max_payload_bytes)
            .map_err(|_| eyre!("compute response payload limit cannot be represented"))?;
        if encoded.len() > base64_encoded_len(payload_bytes)? {
            return Err(eyre!(
                "compute response payload exceeds the declared max_response_bytes"
            ));
        }
    }
    Ok(response)
}
fn ensure_payload_hash(call: &ComputeCall, payload: &[u8]) -> Result<()> {
    let expected = call.request.payload_hash;
    let actual = Hash::new(payload);
    if expected != actual {
        return Err(eyre!(
            "payload hash mismatch: call expects {}, computed {}",
            expected,
            actual
        ));
    }
    Ok(())
}
fn build_gateway_request(call: &ComputeCall, payload: &[u8]) -> Result<GatewayRequest> {
    Ok(GatewayRequest {
        namespace: call.namespace.clone(),
        codec: call.codec,
        ttl_slots: call.ttl_slots,
        gas_limit: call.gas_limit,
        max_response_bytes: call.max_response_bytes,
        determinism: call.determinism,
        execution_class: call.execution_class,
        declared_input_bytes: call.declared_input_bytes,
        declared_input_chunks: call.declared_input_chunks,
        sponsor_budget_cu: call.sponsor_budget_cu,
        price_family: call.price_family.clone(),
        resource_profile: call.resource_profile.clone(),
        auth: call.auth,
        headers: call.request.headers.clone(),
        payload_b64: encode_base64_bounded(payload)?,
    })
}
fn default_manifest() -> ComputeManifest {
    ComputeManifest {
        namespace: compute_defaults::default_namespaces()
            .into_iter()
            .next()
            .expect("default compute namespace"),
        abi_version: ComputeManifest::ABI_VERSION,
        sandbox: ComputeSandboxRules {
            mode: compute_defaults::sandbox_rules().mode,
            randomness: compute_defaults::sandbox_rules().randomness,
            storage: compute_defaults::sandbox_rules().storage,
            deny_nondeterministic_syscalls: true,
            allow_gpu_hints: false,
            allow_tee_hints: false,
        },
        routes: vec![ComputeRoute {
            id: ComputeRouteId::new(
                "payments".parse().expect("route"),
                "quote".parse().expect("route"),
            ),
            entrypoint: "echo".to_string(),
            codecs: vec![ComputeCodec::NoritoJson, ComputeCodec::OctetStream],
            ttl_slots: compute_defaults::default_ttl_slots(),
            gas_budget: compute_defaults::max_gas_per_call(),
            max_request_bytes: std::num::NonZeroU64::new(compute_defaults::MAX_REQUEST_BYTES.0)
                .expect("request cap"),
            max_response_bytes: std::num::NonZeroU64::new(compute_defaults::MAX_RESPONSE_BYTES.0)
                .expect("response cap"),
            determinism: iroha::data_model::compute::ComputeDeterminism::Strict,
            execution_class: iroha::data_model::compute::ComputeExecutionClass::Cpu,
            input_limits: Some(iroha::data_model::compute::ComputeInputLimits {
                max_inline_bytes: std::num::NonZeroU64::new(
                    compute_defaults::MAX_REQUEST_BYTES.0 / 2,
                )
                .expect("inline cap"),
                max_chunks: std::num::NonZeroU32::new(32).expect("chunk count cap"),
                chunk_size_bytes: std::num::NonZeroU64::new(65_536).expect("chunk size cap"),
            }),
            model: Some(iroha::data_model::compute::ComputeModelRef {
                bundle_hash: Hash::new(b"model:echo"),
                path: "models/echo.bin".to_string(),
                expected_bytes: std::num::NonZeroU64::new(1_048_576).expect("model size"),
                chunk_size_bytes: std::num::NonZeroU64::new(131_072).expect("model chunk"),
            }),
            price_family: compute_defaults::default_price_family(),
            resource_profile: compute_defaults::default_resource_profile(),
            auth: ComputeAuthPolicy::Either,
        }],
    }
}
fn execute_entrypoint(
    entrypoint: &str,
    payload: &[u8],
    call: &ComputeCall,
) -> Result<(ComputeOutcome, Option<Vec<u8>>)> {
    let mut response = match entrypoint {
        "echo" | "quote_entry" => Some(copy_compute_payload(payload, false)?),
        "uppercase" => Some(copy_compute_payload(payload, true)?),
        "sha3" => Some(Hash::new(payload).as_ref().to_vec()),
        other => {
            return Err(eyre!(
                "unknown entrypoint `{other}` in compute manifest (only echo/quote_entry/uppercase/sha3 supported for now)"
            ));
        }
    };
    let cycles = (payload.len() as u64)
        .saturating_mul(10_000)
        .saturating_add(5_000);
    if cycles > call.gas_limit.get() {
        response = None;
    }
    if let Some(resp) = &response
        && resp.len() as u64 > call.max_response_bytes.get()
    {
        response = None;
    }
    let outcome = response.as_ref().map_or(
        ComputeOutcome {
            kind: ComputeOutcomeKind::BudgetExhausted,
            response_hash: None,
            response_bytes: None,
            response_codec: None,
        },
        |resp| ComputeOutcome {
            kind: ComputeOutcomeKind::Success,
            response_hash: Some(Hash::new(resp)),
            response_bytes: Some(resp.len() as u64),
            response_codec: Some(call.codec),
        },
    );
    Ok((outcome, response))
}
fn copy_compute_payload(payload: &[u8], uppercase: bool) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(payload.len())
        .map_err(|error| eyre!("failed to reserve compute response storage: {error}"))?;
    if uppercase {
        output.extend(payload.iter().map(u8::to_ascii_uppercase));
    } else {
        output.extend_from_slice(payload);
    }
    Ok(output)
}
fn meter(
    route: &ComputeRoute,
    call: &ComputeCall,
    payload: &[u8],
    response_bytes: Option<&Vec<u8>>,
) -> ComputeMetering {
    let ingress_headers: usize = call
        .request
        .headers
        .iter()
        .map(|(k, v)| k.len() + v.len())
        .sum();
    let egress_bytes = response_bytes.map_or(0, Vec::len);
    let ingress_bytes = payload.len().saturating_add(ingress_headers);
    let cycles = (payload.len() as u64)
        .saturating_mul(10_000)
        .saturating_add(10_000);
    let duration_ms = 1 + (payload.len() as u64 / 256);
    let mut metering = ComputeMetering {
        cycles,
        ingress_bytes: ingress_bytes as u64,
        egress_bytes: egress_bytes as u64,
        duration_ms,
        price_family: call.price_family.clone(),
        charged_units: 0,
    };
    if metering.cycles > route.gas_budget.get() {
        metering.cycles = route.gas_budget.get();
    }
    if metering.egress_bytes > route.max_response_bytes.get() {
        metering.egress_bytes = route.max_response_bytes.get();
    }
    metering
}
fn charge_units(
    price_families: &BTreeMap<iroha::data_model::name::Name, ComputePriceWeights>,
    default_price_family: &iroha::data_model::name::Name,
    metering: &mut ComputeMetering,
) {
    let price_family = price_families
        .get(&metering.price_family)
        .or_else(|| price_families.get(default_price_family))
        .expect("compute price families must be configured");
    metering.charged_units = price_family.charge_units(metering);
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_i18n::{Bundle, Language, Localizer};
    #[test]
    fn simulate_outputs_receipt() {
        let mut ctx = TestContext::new();
        let args = SimulateArgs {
            manifest: workspace_fixture(DEFAULT_MANIFEST),
            call: workspace_fixture(DEFAULT_CALL),
            payload: workspace_fixture(DEFAULT_PAYLOAD),
            payload_inline: None,
            json_out: None,
        };
        Command::Simulate(args).run(&mut ctx).expect("simulate");
        let output = ctx.take_output();
        assert!(
            output.contains("compute"),
            "expected compute output in JSON: {output}"
        );
    }
    #[test]
    fn simulate_rejects_oversized_payload() {
        let args = SimulateArgs {
            manifest: workspace_fixture(DEFAULT_MANIFEST),
            call: workspace_fixture(DEFAULT_CALL),
            payload: workspace_fixture(DEFAULT_PAYLOAD),
            payload_inline: Some("x".repeat(600_000)),
            json_out: None,
        };
        let err = simulate(&args).expect_err("expected payload rejection");
        assert!(err.to_string().contains("max_request_bytes"));
    }
    #[test]
    fn invoke_parses_stubbed_response() {
        let args = InvokeArgs {
            endpoint: "http://localhost:9999".to_string(),
            manifest: workspace_fixture(DEFAULT_MANIFEST),
            call: workspace_fixture(DEFAULT_CALL),
            payload: workspace_fixture(DEFAULT_PAYLOAD),
        };
        let manifest = load_manifest(&args.manifest).expect("manifest");
        let call = load_call(&args.call).expect("call");
        let payload = load_payload(
            &args.payload,
            None,
            compute_defaults::MAX_REQUEST_BYTES.0 as usize,
        )
        .expect("payload");
        let stub = simulate_with_call(&manifest, call.clone(), &payload).expect("simulate");
        let stub_body = json::to_vec(&GatewayResponse {
            receipt: stub.receipt.clone(),
            response_b64: stub.response_b64.clone(),
        })
        .expect("encode stub");
        let output = invoke_with_sender(&args, |_url, _body, _max_response_body_bytes| {
            Ok(stub_body.clone())
        })
        .expect("invoke");
        assert_eq!(output.call.route.method.to_string(), "quote");
        assert_eq!(output.receipt.call.route, call.route);
        assert!(output.response_b64.is_some());
    }
    #[test]
    fn bounded_file_reader_accepts_exact_and_rejects_max_plus_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let exact_path = directory.path().join("exact.bin");
        fs::write(&exact_path, [0_u8; 16]).expect("write exact input");
        assert_eq!(
            read_compute_file_bounded(&exact_path, 16, "test input")
                .expect("exact input")
                .len(),
            16
        );
        let oversized_path = directory.path().join("oversized.bin");
        fs::write(&oversized_path, [0_u8; 17]).expect("write oversized input");
        let error = read_compute_file_bounded(&oversized_path, 16, "test input")
            .expect_err("max plus one must fail");
        assert!(error.to_string().contains("first-release limit"));
    }
    #[test]
    fn response_reader_preflights_declared_length_and_probes_growth() {
        struct PanicReader;
        impl std::io::Read for PanicReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                panic!("declared oversize must fail before reading")
            }
        }
        let declared = read_compute_response_body_bounded(&mut PanicReader, Some(17), 16)
            .expect_err("declared max plus one must fail");
        assert!(declared.to_string().contains("Content-Length"));
        let mut exact = std::io::Cursor::new([0_u8; 16]);
        assert_eq!(
            read_compute_response_body_bounded(&mut exact, None, 16)
                .expect("exact response")
                .len(),
            16
        );
        let mut oversized = std::io::Cursor::new([0_u8; 17]);
        let growth = read_compute_response_body_bounded(&mut oversized, None, 16)
            .expect_err("chunked max plus one must fail");
        assert!(growth.to_string().contains("first-release limit"));
    }
    #[test]
    fn response_payload_text_cannot_exceed_declared_payload_cap() {
        let manifest = default_manifest();
        let payload_cap = 3_u64;
        let body_cap = gateway_response_body_limit(payload_cap).expect("response body cap");
        let call = load_call(&workspace_fixture(DEFAULT_CALL)).expect("call");
        let payload = load_payload(
            &workspace_fixture(DEFAULT_PAYLOAD),
            None,
            compute_defaults::MAX_REQUEST_BYTES.0 as usize,
        )
        .expect("payload");
        let receipt = simulate_with_call(&manifest, call, &payload)
            .expect("simulate")
            .receipt;
        let body = json::to_vec(&GatewayResponse {
            receipt,
            response_b64: Some("AAAAA".to_string()),
        })
        .expect("encode response");
        let error = decode_gateway_response_bounded(&body, payload_cap, body_cap)
            .expect_err("base64 text over the declared cap must fail");
        assert!(error.to_string().contains("max_response_bytes"));
    }
    fn workspace_fixture(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }
    struct TestContext {
        output: Vec<u8>,
        i18n: Localizer,
    }
    impl TestContext {
        fn new() -> Self {
            Self {
                output: Vec::new(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
        fn take_output(&mut self) -> String {
            let out = String::from_utf8(self.output.clone()).unwrap_or_default();
            self.output.clear();
            out
        }
    }
    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            use iroha::config::LoadPath;
            use std::{fs, sync::OnceLock};
            static CONFIG: OnceLock<iroha::config::Config> = OnceLock::new();
            CONFIG.get_or_init(|| {
                static CONFIG_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
                let path = CONFIG_PATH.get_or_init(|| {
                    let target = std::env::temp_dir().join("compute_test_config.toml");
                    let body = r#"
chain = "00000000-0000-0000-0000-000000000000"
torii_url = "http://127.0.0.1:8080/"

[basic_auth]
web_login = "alice"
password = "password"

[account]
domain = "wonderland"
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
private_key = "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"

[transaction]
time_to_live_ms = 100_000
status_timeout_ms = 100_000
nonce = false
"#;
                    let _ = fs::write(&target, body);
                    target
                });
                iroha::config::Config::load(LoadPath::Explicit(path.clone()))
                    .expect("load compute test config")
            })
        }
        fn transaction_metadata(&self) -> Option<&iroha::data_model::metadata::Metadata> {
            None
        }
        fn input_instructions(&self) -> bool {
            false
        }
        fn output_instructions(&self) -> bool {
            false
        }
        fn i18n(&self) -> &Localizer {
            &self.i18n
        }
        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            let bytes = json::to_vec(data)?;
            self.output.extend_from_slice(&bytes);
            Ok(())
        }
        fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
            use std::io::Write as _;
            writeln!(&mut self.output, "{data}")?;
            Ok(())
        }
    }
}
