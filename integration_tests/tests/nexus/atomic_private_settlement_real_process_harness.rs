use iroha::data_model::block::consensus_v2::SumeragiV2GenesisContextParameters;
use iroha_test_network::{
    ConsensusMessageControlAction, ConsensusMessageControlKind, ConsensusMessageControlRule,
    NativeAmxFaultPhase, PrivateSettlementRouteControlAction, PrivateSettlementRouteControlPhase,
};
use norito::json::Value as HarnessJsonValue;
use sha2::{Digest as Sha2Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    num::NonZeroU64,
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const HARNESS_REQUEST_ENV: &str = "APS_REAL_PROCESS_REQUEST";
const HARNESS_RESULT_ENV: &str = "APS_REAL_PROCESS_RESULT";
const HARNESS_REQUEST_SHA_ENV: &str = "APS_REAL_PROCESS_REQUEST_SHA256";
const HARNESS_VALIDATOR_SHA_ENV: &str = "APS_REAL_PROCESS_VALIDATOR_SHA256";
const HARNESS_EVIDENCE_DIR_ENV: &str = "APS_REAL_PROCESS_EVIDENCE_DIR";
const COORDINATOR_ROOT_ENV: &str = "APS_REAL_PROCESS_COORDINATOR_ROOT";
const COORDINATOR_COMMAND_FILE: &str = "command.json";
const COORDINATOR_ACK_FILE: &str = "ack.json";
const COORDINATOR_CONFIG_FILE: &str = "client.toml";
const COORDINATOR_SHUTDOWN_FILE: &str = "shutdown";
const HARNESS_MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
const FAULT_CONTROL_EVIDENCE_FILE: &str = "fault-control.jsonl";
const FAULT_OBSERVATION_EVIDENCE_FILE: &str = "fault-observations.jsonl";
const FAULT_ROUTE_MATCHES: u64 = 20;
const FAULT_BUNDLE_EXPIRY_BLOCKS: u64 = 96;
const FAULT_CONTROL_TIMEOUT: Duration = Duration::from_secs(60);
const FAULT_CONTINUOUS_OBSERVATION_DOMAIN_V1: &[u8] =
    b"iroha:aps-fault-continuous-observation:v1\0";

#[derive(Debug, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RealProcessBenchmarkPayloadV1 {
    profile: String,
    warmup: bool,
    stages: Vec<String>,
    resources: Vec<String>,
}

#[derive(Debug, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RealProcessBenchmarkRequestV1 {
    version: u8,
    protocol: String,
    request_id: String,
    invocation_nonce: String,
    kind: String,
    commit: String,
    hardware_sha256: String,
    hardware_profile_sha256: String,
    configuration_sha256: String,
    participants: usize,
    validators_per_dataspace: usize,
    global_validators: usize,
    quorum: String,
    mandatory_signed_rs16_da_rbc: bool,
    minimum_signed_rs16_da_observations: u64,
    authenticated_message_control: bool,
    seed: u64,
    run: u64,
    configuration: HarnessJsonValue,
    payload: RealProcessBenchmarkPayloadV1,
}

#[derive(Debug, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RealProcessPrepareQcNormalizationRequestV1 {
    first_signer_subset: Vec<u8>,
    second_signer_subset: Vec<u8>,
    accept_equivalent_subsets_only_for_identical_body: bool,
    bind_authority_indices: bool,
    bind_every_signed_body: bool,
    reject_changed_certified_body: bool,
}

#[derive(Debug, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RealProcessFaultPayloadV1 {
    loss_phases: Vec<String>,
    loss_percentages: Vec<u8>,
    phase_cuts: Vec<String>,
    crash_boundaries: Vec<String>,
    committee_validator_restarts: Vec<usize>,
    restart_coordinator: bool,
    restart_global_node: bool,
    maximum_simultaneously_unavailable_per_committee: usize,
    continuous_atomicity_checks: bool,
    prepare_qc_normalization: RealProcessPrepareQcNormalizationRequestV1,
}

#[derive(Debug, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RealProcessFaultRequestV1 {
    version: u8,
    protocol: String,
    request_id: String,
    invocation_nonce: String,
    kind: String,
    commit: String,
    hardware_sha256: String,
    hardware_profile_sha256: String,
    configuration_sha256: String,
    participants: usize,
    validators_per_dataspace: usize,
    global_validators: usize,
    quorum: String,
    mandatory_signed_rs16_da_rbc: bool,
    minimum_signed_rs16_da_observations: u64,
    authenticated_message_control: bool,
    seed: u64,
    run: u64,
    configuration: HarnessJsonValue,
    payload: RealProcessFaultPayloadV1,
}

enum RealProcessBoundRequestV1 {
    Benchmark(RealProcessBenchmarkRequestV1),
    Fault(RealProcessFaultRequestV1),
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct RealProcessInventoryRowV1 {
    role: String,
    #[norito(required)]
    dataspace_ordinal: Option<u64>,
    #[norito(required)]
    validator_ordinal: Option<u64>,
    pid: u32,
    executable_sha256: String,
    revision: String,
    health_observed: bool,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct RealProcessBenchmarkResultPayloadV1 {
    stages_ms: HarnessJsonValue,
    throughput_bundles_per_second: f64,
    cpu_seconds: f64,
    peak_rss_bytes: u64,
    network_bytes: u64,
    proof_bytes: u64,
    receipt_bytes: u64,
    storage_growth_bytes: u64,
    finalized_receipt_observed: bool,
    successful_leg_applications: usize,
    each_leg_applied_exactly_once: bool,
    partial_visible_observations: u64,
    partial_spendable_observations: u64,
}

#[derive(Debug, norito::JsonSerialize)]
struct RealProcessBenchmarkResultV1 {
    version: u8,
    protocol: String,
    request_id: String,
    invocation_nonce: String,
    request_sha256: String,
    commit: String,
    participants: usize,
    mandatory_signed_rs16_da_rbc: bool,
    signed_rs16_da_observations: u64,
    authenticated_message_control: bool,
    process_inventory: Vec<RealProcessInventoryRowV1>,
    payload: RealProcessBenchmarkResultPayloadV1,
}

#[derive(Debug, norito::JsonSerialize)]
struct RealProcessFaultResultV1 {
    version: u8,
    protocol: String,
    request_id: String,
    invocation_nonce: String,
    request_sha256: String,
    commit: String,
    participants: usize,
    mandatory_signed_rs16_da_rbc: bool,
    signed_rs16_da_observations: u64,
    authenticated_message_control: bool,
    process_inventory: Vec<RealProcessInventoryRowV1>,
    payload: HarnessJsonValue,
}

#[derive(Clone, Debug, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CoordinatorCommandV1 {
    format_version: u8,
    revision: u64,
    operation: String,
    committee_endpoints: Vec<Vec<String>>,
    #[norito(required)]
    manifest: Option<AtomicPrivateSettlementV1>,
    authority_catalog: Vec<PrivateSettlementCommitteeAuthorityV1>,
    deltas: Vec<PrivateSettlementDeltaV1>,
    #[norito(required)]
    barrier: Option<iroha::data_model::nexus::PrivateSettlementPrepareBarrierV1>,
}

#[derive(Clone, Debug, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CoordinatorAckV1 {
    format_version: u8,
    revision: u64,
    command_sha256: String,
    pid: u32,
    operation: String,
    #[norito(required)]
    barrier: Option<iroha::data_model::nexus::PrivateSettlementPrepareBarrierV1>,
    commit_certificates: Vec<iroha::data_model::nexus::PrivateSettlementPhaseCertificateV1>,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultControlOccurrenceV1 {
    control_type: String,
    #[norito(required)]
    peer_index: Option<usize>,
    command_sha256: String,
    command_hex: String,
    acknowledgement_sha256: String,
    acknowledgement_hex: String,
    #[norito(required)]
    before_pid: Option<u32>,
    #[norito(required)]
    after_pid: Option<u32>,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultControlRecordV1 {
    record: String,
    bundle_id: String,
    participants: usize,
    seed: u64,
    run: u64,
    collection: String,
    trial_index: usize,
    controls: Vec<FaultControlOccurrenceV1>,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultStateObservationV1 {
    peer_index: usize,
    response_sha256: String,
    response_hex: String,
    height: u64,
    commitment: String,
    ledger_commitment: String,
    staged_lock_commitment: String,
    counts: HarnessJsonValue,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultStateSnapshotV1 {
    label: String,
    validators: Vec<FaultStateObservationV1>,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultObservationRecordV1 {
    record: String,
    bundle_id: String,
    participants: usize,
    seed: u64,
    run: u64,
    collection: String,
    trial_index: usize,
    expected_after_state: String,
    continuous_checks: u64,
    continuous_observations: Vec<FaultContinuousObservationSummaryV1>,
    partial_visibility_observed: bool,
    partial_spendable_observations: u64,
    snapshots: Vec<FaultStateSnapshotV1>,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultContinuousObservationSummaryV1 {
    peer_index: usize,
    check_count: u64,
    first_response_sha256: String,
    last_response_sha256: String,
    response_chain_sha256: String,
    baseline_observations: u64,
    finalized_observations: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FaultStateIdentityV1 {
    ledger_commitment: String,
    staged_lock_commitment: String,
    counts: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FaultLedgerIdentityV1 {
    ledger_commitment: String,
    counts: String,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultPrepareQcNormalizationV1 {
    first_signer_subset: Vec<u8>,
    second_signer_subset: Vec<u8>,
    certified_body_sha256: String,
    first_qc_sha256: String,
    second_qc_sha256: String,
    first_normalized_barrier_sha256: String,
    second_normalized_barrier_sha256: String,
    equivalent_subsets_accepted: bool,
    changed_body_rejected: bool,
    authority_index_binding_verified: bool,
    signed_body_binding_verified: bool,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultRestartCommandV1 {
    format_version: u8,
    revision: u64,
    operation: String,
    peer_index: usize,
    before_pid: u32,
}

#[derive(Clone, Debug, norito::JsonSerialize)]
struct FaultRestartAckV1 {
    format_version: u8,
    revision: u64,
    command_sha256: String,
    operation: String,
    peer_index: usize,
    before_pid: u32,
    after_pid: u32,
    health_observed: bool,
}

struct FaultStoppedPeerV1 {
    peer: NetworkPeer,
    peer_index: usize,
    before_pid: u32,
    revision: u64,
    command_bytes: Vec<u8>,
}

struct FaultTrialDraftV1 {
    collection: &'static str,
    trial_index: usize,
    bundle_id: String,
    expected_after_state: &'static str,
    controls: Vec<FaultControlOccurrenceV1>,
    before: FaultStateSnapshotV1,
    nonfinalized: FaultStateSnapshotV1,
    continuous_observations: Vec<FaultContinuousObservationSummaryV1>,
}

struct FaultPreparedBundleV1 {
    manifest: AtomicPrivateSettlementV1,
    prepared: Vec<PreparedLeg>,
    materials: Vec<PrivateSettlementProvisionalLegMaterialV1>,
    authorities: Vec<PrivateSettlementCommitteeAuthorityV1>,
    deltas: Vec<PrivateSettlementDeltaV1>,
}

#[derive(Clone, Copy, Debug)]
struct ProcessResourceSample {
    cpu_seconds: f64,
    rss_bytes: u64,
}

const PRIVATE_BENCHMARK_STAGES: &[&str] = &[
    "proof_generation",
    "restricted_upload_availability",
    "auditor_response",
    "committee_verification",
    "prepare",
    "commit",
    "global_finality",
    "end_to_end",
];
const TRANSPARENT_CONTROL_BENCHMARK_STAGES: &[&str] = &["global_finality", "end_to_end"];

fn benchmark_stages(profile: &str) -> Result<&'static [&'static str]> {
    match profile {
        "private" => Ok(PRIVATE_BENCHMARK_STAGES),
        "transparent_control" => Ok(TRANSPARENT_CONTROL_BENCHMARK_STAGES),
        _ => Err(eyre!("unsupported real-process benchmark profile")),
    }
}

fn lowercase_digest(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && !value.bytes().all(|byte| byte == b'0')
}

fn harness_json_object_u64(value: &HarnessJsonValue, outer: &str, inner: &str) -> Option<u64> {
    value.get(outer)?.get(inner)?.as_u64()
}

fn harness_json_object_bool(value: &HarnessJsonValue, outer: &str, inner: &str) -> Option<bool> {
    value.get(outer)?.get(inner)?.as_bool()
}

#[allow(clippy::too_many_arguments)]
fn validate_real_process_request_common(
    version: u8,
    protocol: &str,
    kind: &str,
    request_id: &str,
    invocation_nonce: &str,
    commit: &str,
    hardware_sha256: &str,
    hardware_profile_sha256: &str,
    configuration_sha256: &str,
    participants: usize,
    validators_per_dataspace: usize,
    global_validators: usize,
    quorum: &str,
    mandatory_signed_rs16_da_rbc: bool,
    minimum_signed_rs16_da_observations: u64,
    authenticated_message_control: bool,
    configuration: &HarnessJsonValue,
) -> Result<()> {
    let shape = TopologyShape::new(participants);
    shape.validate()?;
    ensure!(
        version == 1
            && protocol == "AtomicPrivateSettlementV1"
            && matches!(kind, "benchmark" | "fault"),
        "Rust harness supports only AtomicPrivateSettlementV1 release requests"
    );
    ensure!(
        validators_per_dataspace == VALIDATORS_PER_LANE
            && global_validators == VALIDATORS_PER_LANE
            && quorum == "3-of-4"
            && mandatory_signed_rs16_da_rbc
            && authenticated_message_control,
        "request weakens the required real-process topology"
    );
    ensure!(
        minimum_signed_rs16_da_observations
            == u64::try_from(shape.peer_count()).expect("peer count fits u64"),
        "signed RS16 observation minimum must cover every validator"
    );
    ensure!(
        lowercase_digest(request_id, &[64])
            && lowercase_digest(invocation_nonce, &[64])
            && lowercase_digest(commit, &[40, 64])
            && lowercase_digest(hardware_sha256, &[64])
            && lowercase_digest(hardware_profile_sha256, &[64])
            && lowercase_digest(configuration_sha256, &[64]),
        "request contains a malformed binding digest"
    );
    ensure!(
        configuration
            .get("version")
            .and_then(HarnessJsonValue::as_u64)
            == Some(1)
            && configuration
                .get("protocol")
                .and_then(HarnessJsonValue::as_str)
                == Some("AtomicPrivateSettlementV1")
            && configuration
                .get("participants")
                .and_then(HarnessJsonValue::as_u64)
                == u64::try_from(participants).ok()
            && harness_json_object_u64(configuration, "topology", "validators_per_dataspace",)
                == Some(4)
            && harness_json_object_u64(configuration, "topology", "global_validators",) == Some(4)
            && harness_json_object_bool(configuration, "consensus", "mandatory_signed_rs16_da_rbc",)
                == Some(true)
            && harness_json_object_bool(
                configuration,
                "consensus",
                "authenticated_message_control",
            ) == Some(true),
        "embedded configuration does not bind the required topology"
    );
    Ok(())
}

fn validate_real_process_request(request: &RealProcessBenchmarkRequestV1) -> Result<()> {
    validate_real_process_request_common(
        request.version,
        &request.protocol,
        &request.kind,
        &request.request_id,
        &request.invocation_nonce,
        &request.commit,
        &request.hardware_sha256,
        &request.hardware_profile_sha256,
        &request.configuration_sha256,
        request.participants,
        request.validators_per_dataspace,
        request.global_validators,
        &request.quorum,
        request.mandatory_signed_rs16_da_rbc,
        request.minimum_signed_rs16_da_observations,
        request.authenticated_message_control,
        &request.configuration,
    )?;
    ensure!(
        request.kind == "benchmark"
            && matches!(
                request.payload.profile.as_str(),
                "private" | "transparent_control"
            ),
        "Rust harness supports only canonical benchmark profiles"
    );
    ensure!(
        request.payload.stages
            == benchmark_stages(&request.payload.profile)?
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        "request contains a non-canonical profile stage inventory"
    );
    ensure!(
        request.payload.resources
            == [
                "throughput_bundles_per_second",
                "cpu_seconds",
                "peak_rss_bytes",
                "network_bytes",
                "proof_bytes",
                "receipt_bytes",
                "storage_growth_bytes",
            ],
        "request contains a non-canonical resource inventory"
    );
    let _ = request.payload.warmup;
    let _ = request.run;
    Ok(())
}

fn validate_real_process_fault_request(request: &RealProcessFaultRequestV1) -> Result<()> {
    validate_real_process_request_common(
        request.version,
        &request.protocol,
        &request.kind,
        &request.request_id,
        &request.invocation_nonce,
        &request.commit,
        &request.hardware_sha256,
        &request.hardware_profile_sha256,
        &request.configuration_sha256,
        request.participants,
        request.validators_per_dataspace,
        request.global_validators,
        &request.quorum,
        request.mandatory_signed_rs16_da_rbc,
        request.minimum_signed_rs16_da_observations,
        request.authenticated_message_control,
        &request.configuration,
    )?;
    let normalization = &request.payload.prepare_qc_normalization;
    ensure!(
        request.kind == "fault"
            && request.payload.loss_phases == ["restricted_da", "prepare", "commit"]
            && request.payload.loss_percentages == [5, 10, 20]
            && request.payload.phase_cuts
                == [
                    "da_before_availability_qc",
                    "prepare_before_complete_barrier",
                    "commit_before_complete_barrier",
                    "carrier_before_global_finality",
                ]
            && request.payload.crash_boundaries
                == [
                    "sidecar_fsync",
                    "staged_delta_fsync",
                    "prepare_qc",
                    "commit_qc",
                    "kura_append",
                    "wsv_application",
                    "receipt_publication",
                ]
            && request.payload.committee_validator_restarts
                == (0..request.participants).collect::<Vec<_>>()
            && request.payload.restart_coordinator
            && request.payload.restart_global_node
            && request
                .payload
                .maximum_simultaneously_unavailable_per_committee
                == 1
            && request.payload.continuous_atomicity_checks
            && normalization.first_signer_subset == [0, 1, 2]
            && normalization.second_signer_subset == [0, 1, 3]
            && normalization.accept_equivalent_subsets_only_for_identical_body
            && normalization.bind_authority_indices
            && normalization.bind_every_signed_body
            && normalization.reject_changed_certified_body,
        "fault request differs from the exact release matrix"
    );
    let _ = request.run;
    Ok(())
}

fn read_bound_real_process_request() -> Result<(RealProcessBoundRequestV1, String)> {
    let path = PathBuf::from(
        std::env::var(HARNESS_REQUEST_ENV).wrap_err("missing real-process request path")?,
    );
    let metadata = fs::symlink_metadata(&path).wrap_err("inspect real-process request")?;
    ensure!(
        metadata.file_type().is_file(),
        "request is not a regular file"
    );
    ensure!(
        usize::try_from(metadata.len()).is_ok_and(|len| len <= HARNESS_MAX_JSON_BYTES),
        "request exceeds the Rust JSON bound"
    );
    let mut file = File::open(&path).wrap_err("open real-process request")?;
    let mut raw = Vec::new();
    file.by_ref()
        .take(u64::try_from(HARNESS_MAX_JSON_BYTES + 1).expect("bound fits u64"))
        .read_to_end(&mut raw)
        .wrap_err("read real-process request")?;
    ensure!(raw.len() <= HARNESS_MAX_JSON_BYTES, "request is too large");
    let request_sha = hex::encode(Sha256::digest(&raw));
    let expected_sha =
        std::env::var(HARNESS_REQUEST_SHA_ENV).wrap_err("missing real-process request digest")?;
    ensure!(
        lowercase_digest(&expected_sha, &[64]) && request_sha == expected_sha,
        "request bytes do not match the launcher binding"
    );
    let text = std::str::from_utf8(&raw).wrap_err("request is not UTF-8")?;
    let value: HarnessJsonValue =
        norito::json::from_str(text).wrap_err("decode strict real-process request")?;
    let kind = value
        .get("kind")
        .and_then(HarnessJsonValue::as_str)
        .ok_or_else(|| eyre!("real-process request lacks kind"))?;
    let request = match kind {
        "benchmark" => {
            let request: RealProcessBenchmarkRequestV1 =
                norito::json::from_value(value).wrap_err("decode strict benchmark request")?;
            validate_real_process_request(&request)?;
            RealProcessBoundRequestV1::Benchmark(request)
        }
        "fault" => {
            let request: RealProcessFaultRequestV1 =
                norito::json::from_value(value).wrap_err("decode strict fault request")?;
            validate_real_process_fault_request(&request)?;
            RealProcessBoundRequestV1::Fault(request)
        }
        _ => return Err(eyre!("unsupported real-process request kind")),
    };
    Ok((request, request_sha))
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000.0
}

fn sha256_regular_file(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path).wrap_err("inspect executable")?;
    ensure!(
        metadata.file_type().is_file(),
        "executable is not a regular file"
    );
    let mut file = File::open(path).wrap_err("open executable")?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).wrap_err("hash executable")?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn executable_for_pid(pid: u32) -> Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        return fs::read_link(format!("/proc/{pid}/exe"))
            .wrap_err_with(|| format!("resolve executable for PID {pid}"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "command="])
            .output()
            .wrap_err("run ps for executable identity")?;
        ensure!(output.status.success(), "ps could not inspect PID {pid}");
        let command = std::str::from_utf8(&output.stdout)
            .wrap_err("ps command output is not UTF-8")?
            .trim();
        let executable = command
            .split_whitespace()
            .next()
            .ok_or_else(|| eyre!("ps returned no executable for PID {pid}"))?;
        return fs::canonicalize(executable)
            .wrap_err_with(|| format!("resolve executable for PID {pid}"));
    }
}

fn parse_ps_cpu_time(value: &str) -> Result<f64> {
    let (days, clock) = value
        .split_once('-')
        .map_or((0_u64, value), |(days, clock)| {
            (days.parse::<u64>().unwrap_or(u64::MAX), clock)
        });
    ensure!(days != u64::MAX, "invalid ps CPU day count");
    let components = clock.split(':').collect::<Vec<_>>();
    ensure!((2..=3).contains(&components.len()), "invalid ps CPU time");
    let (hours, minutes, seconds) = if components.len() == 3 {
        (
            components[0].parse::<u64>()?,
            components[1].parse::<u64>()?,
            components[2].parse::<f64>()?,
        )
    } else {
        (
            0,
            components[0].parse::<u64>()?,
            components[1].parse::<f64>()?,
        )
    };
    Ok(days as f64 * 86_400.0 + hours as f64 * 3_600.0 + minutes as f64 * 60.0 + seconds)
}

fn sample_process_resources(pids: &[u32]) -> Result<ProcessResourceSample> {
    ensure!(!pids.is_empty(), "cannot sample an empty process set");
    let joined = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let output = Command::new("ps")
        .args(["-p", &joined, "-o", "pid=,rss=,time="])
        .output()
        .wrap_err("sample process resources")?;
    ensure!(output.status.success(), "ps resource sampling failed");
    let text = std::str::from_utf8(&output.stdout).wrap_err("ps output is not UTF-8")?;
    let mut observed = BTreeSet::new();
    let mut cpu_seconds = 0.0;
    let mut rss_kib = 0_u64;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        ensure!(fields.len() == 3, "unexpected ps resource row");
        let pid = fields[0].parse::<u32>()?;
        ensure!(pids.contains(&pid), "ps returned an unrequested PID");
        ensure!(observed.insert(pid), "ps returned a duplicate PID");
        rss_kib = rss_kib
            .checked_add(fields[1].parse::<u64>()?)
            .ok_or_else(|| eyre!("RSS total overflow"))?;
        cpu_seconds += parse_ps_cpu_time(fields[2])?;
    }
    ensure!(
        observed.len() == pids.len(),
        "one measured process disappeared"
    );
    Ok(ProcessResourceSample {
        cpu_seconds,
        rss_bytes: rss_kib
            .checked_mul(1_024)
            .ok_or_else(|| eyre!("RSS byte total overflow"))?,
    })
}

struct ProcessResourceSampler {
    stop: Arc<AtomicBool>,
    peak_rss: Arc<AtomicU64>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ProcessResourceSampler {
    fn start(pids: Vec<u32>, initial_rss_bytes: u64) -> Result<Self> {
        ensure!(!pids.is_empty(), "cannot sample an empty process set");
        let stop = Arc::new(AtomicBool::new(false));
        let peak_rss = Arc::new(AtomicU64::new(initial_rss_bytes));
        let sampler_stop = Arc::clone(&stop);
        let sampler_peak = Arc::clone(&peak_rss);
        let handle = thread::Builder::new()
            .name("aps-real-process-rss-sampler".to_owned())
            .spawn(move || {
                while !sampler_stop.load(Ordering::Relaxed) {
                    if let Ok(sample) = sample_process_resources(&pids) {
                        sampler_peak.fetch_max(sample.rss_bytes, Ordering::Relaxed);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            })?;
        Ok(Self {
            stop,
            peak_rss,
            handle: Some(handle),
        })
    }

    fn stop_and_join(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| eyre!("RSS sampler thread panicked"))?;
        }
        Ok(())
    }

    fn finish(mut self, final_rss_bytes: u64) -> Result<u64> {
        self.peak_rss.fetch_max(final_rss_bytes, Ordering::Relaxed);
        self.stop_and_join()?;
        Ok(self.peak_rss.load(Ordering::Relaxed))
    }
}

impl Drop for ProcessResourceSampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(target_os = "linux")]
fn loopback_bytes() -> Result<u64> {
    let receive = fs::read_to_string("/sys/class/net/lo/statistics/rx_bytes")?
        .trim()
        .parse::<u64>()?;
    let transmit = fs::read_to_string("/sys/class/net/lo/statistics/tx_bytes")?
        .trim()
        .parse::<u64>()?;
    receive
        .checked_add(transmit)
        .ok_or_else(|| eyre!("loopback byte counter overflow"))
}

#[cfg(target_os = "macos")]
fn loopback_bytes() -> Result<u64> {
    let output = Command::new("netstat")
        .args(["-ibn", "-I", "lo0"])
        .output()
        .wrap_err("sample loopback counters")?;
    ensure!(output.status.success(), "netstat loopback sampling failed");
    let text = std::str::from_utf8(&output.stdout).wrap_err("netstat output is not UTF-8")?;
    let mut input_index = None;
    let mut output_index = None;
    let mut maximum = None;
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.first() == Some(&"Name") {
            input_index = fields.iter().position(|field| *field == "Ibytes");
            output_index = fields.iter().position(|field| *field == "Obytes");
            continue;
        }
        if fields.first() != Some(&"lo0") {
            continue;
        }
        let (Some(input), Some(output)) = (input_index, output_index) else {
            continue;
        };
        let total = fields
            .get(input)
            .ok_or_else(|| eyre!("netstat lacks Ibytes"))?
            .parse::<u64>()?
            .checked_add(
                fields
                    .get(output)
                    .ok_or_else(|| eyre!("netstat lacks Obytes"))?
                    .parse::<u64>()?,
            )
            .ok_or_else(|| eyre!("loopback byte counter overflow"))?;
        maximum = Some(maximum.map_or(total, |current: u64| current.max(total)));
    }
    maximum.ok_or_else(|| eyre!("netstat returned no loopback byte counter"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn loopback_bytes() -> Result<u64> {
    Err(eyre!(
        "real network-byte measurement is unsupported on this operating system"
    ))
}

fn regular_tree_bytes(root: &Path) -> Result<u64> {
    let mut total = 0_u64;
    let mut stack = vec![root.to_path_buf()];
    let mut entries = 0_usize;
    while let Some(path) = stack.pop() {
        let metadata = fs::symlink_metadata(&path)
            .wrap_err_with(|| format!("inspect storage path {}", path.display()))?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "storage measurement encountered a symbolic link"
        );
        if metadata.is_file() {
            total = total
                .checked_add(metadata.len())
                .ok_or_else(|| eyre!("storage measurement overflow"))?;
            continue;
        }
        ensure!(metadata.is_dir(), "storage contains a special file");
        for entry in fs::read_dir(&path)? {
            stack.push(entry?.path());
            entries += 1;
            ensure!(entries <= 1_000_000, "storage tree exceeds entry bound");
        }
    }
    Ok(total)
}

fn network_storage_bytes(network: &Network) -> Result<u64> {
    network.peers().iter().try_fold(0_u64, |total, peer| {
        total
            .checked_add(regular_tree_bytes(&peer.kura_store_dir())?)
            .ok_or_else(|| eyre!("network storage total overflow"))
    })
}

fn verify_controller_readiness(network: &Network, runtime: &tokio::runtime::Runtime) -> Result<()> {
    for peer in network.peers() {
        let control = peer
            .consensus_message_control()
            .ok_or_else(|| eyre!("peer {} lacks authenticated message control", peer.id()))?;
        let acknowledgement = runtime
            .block_on(control.wait_until_ready(Duration::from_secs(30)))
            .wrap_err_with(|| format!("wait for controller on {}", peer.id()))?;
        ensure!(
            acknowledgement.revision == 1
                && acknowledgement.rules.is_empty()
                && acknowledgement.held.is_empty()
                && acknowledgement.release_pending.is_empty()
                && acknowledgement.in_flight.is_none()
                && acknowledgement.last_error.is_none()
                && !acknowledgement.fatal
                && !acknowledgement.draining,
            "peer {} did not acknowledge a clean controller state",
            peer.id()
        );
    }
    Ok(())
}

fn collect_process_inventory(
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    shape: TopologyShape,
    revision: &str,
    coordinator: &CoordinatorProcessV1,
) -> Result<Vec<RealProcessInventoryRowV1>> {
    let validator_sha =
        std::env::var(HARNESS_VALIDATOR_SHA_ENV).wrap_err("missing validator executable digest")?;
    ensure!(
        lowercase_digest(&validator_sha, &[64]),
        "validator executable digest is malformed"
    );
    let coordinator_pid = coordinator.pid;
    let mut rows = vec![RealProcessInventoryRowV1 {
        role: "coordinator".to_owned(),
        dataspace_ordinal: None,
        validator_ordinal: None,
        pid: coordinator_pid,
        executable_sha256: coordinator.executable_sha256.clone(),
        revision: revision.to_owned(),
        health_observed: sample_process_resources(&[coordinator_pid]).is_ok(),
    }];
    for (index, peer) in network.peers().iter().enumerate() {
        let pid = runtime
            .block_on(peer.process_id())
            .ok_or_else(|| eyre!("peer {} has no live child PID", peer.id()))?;
        let actual_path = executable_for_pid(pid)?;
        let actual_sha = sha256_regular_file(&actual_path)?;
        ensure!(
            actual_sha == validator_sha,
            "peer {} executable differs from the launcher-bound daemon",
            peer.id()
        );
        let lane = index / VALIDATORS_PER_LANE;
        rows.push(RealProcessInventoryRowV1 {
            role: if lane == 0 {
                "global_validator".to_owned()
            } else {
                "dataspace_validator".to_owned()
            },
            dataspace_ordinal: (lane != 0).then(|| u64::try_from(lane - 1).unwrap()),
            validator_ordinal: Some(
                u64::try_from(index % VALIDATORS_PER_LANE).expect("validator ordinal fits u64"),
            ),
            pid,
            executable_sha256: actual_sha,
            revision: revision.to_owned(),
            health_observed: peer.is_running() && peer.client().get_status().is_ok(),
        });
    }
    ensure!(
        rows.len() == shape.peer_count() + 1
            && rows.iter().all(|row| row.health_observed)
            && rows
                .iter()
                .map(|row| row.pid)
                .collect::<BTreeSet<_>>()
                .len()
                == rows.len(),
        "process inventory is incomplete or unhealthy"
    );
    Ok(rows)
}

fn canonical_harness_json_bytes<T: norito::json::JsonSerialize>(value: &T) -> Result<Vec<u8>> {
    let value = norito::json::to_value(value).wrap_err("encode harness evidence value")?;
    norito::json::to_string(&value)
        .map(String::into_bytes)
        .wrap_err("encode canonical harness evidence JSON")
}

fn canonical_harness_json_value_bytes(value: &HarnessJsonValue) -> Result<Vec<u8>> {
    norito::json::to_string(value)
        .map(String::into_bytes)
        .wrap_err("encode canonical harness evidence JSON value")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn fault_control_occurrence(
    control_type: impl Into<String>,
    peer_index: Option<usize>,
    command_bytes: Vec<u8>,
    acknowledgement_bytes: Vec<u8>,
    before_pid: Option<u32>,
    after_pid: Option<u32>,
) -> Result<FaultControlOccurrenceV1> {
    ensure!(
        !command_bytes.is_empty()
            && !acknowledgement_bytes.is_empty()
            && canonical_harness_json_value_bytes(&norito::json::from_slice(&command_bytes)?)?
                == command_bytes
            && canonical_harness_json_value_bytes(&norito::json::from_slice(
                &acknowledgement_bytes,
            )?)? == acknowledgement_bytes,
        "fault control evidence is empty or non-canonical"
    );
    if let (Some(before), Some(after)) = (before_pid, after_pid) {
        ensure!(before != after, "fault restart reused one process id");
    }
    Ok(FaultControlOccurrenceV1 {
        control_type: control_type.into(),
        peer_index,
        command_sha256: sha256_hex(&command_bytes),
        command_hex: hex::encode(command_bytes),
        acknowledgement_sha256: sha256_hex(&acknowledgement_bytes),
        acknowledgement_hex: hex::encode(acknowledgement_bytes),
        before_pid,
        after_pid,
    })
}

fn fault_record_id(
    participants: usize,
    seed: u64,
    run: u64,
    collection: &str,
    trial_index: usize,
) -> String {
    format!("n{participants}:s{seed}:r{run}:{collection}:{trial_index}")
}

fn fault_derivation_bytes(
    request: &RealProcessFaultRequestV1,
    bundle_ordinal: usize,
    leg_ordinal: usize,
    purpose: &[u8],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"iroha:atomic-private-settlement:fault-campaign:v1\0");
    digest.update(request.seed.to_le_bytes());
    digest.update(request.run.to_le_bytes());
    digest.update(
        u64::try_from(bundle_ordinal)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    digest.update(u64::try_from(leg_ordinal).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(purpose);
    digest.finalize().into()
}

fn fault_governed_legs(
    request: &RealProcessFaultRequestV1,
    bundle_ordinal: usize,
    routes: &[PrivateSettlementRouteV1],
    authority_context_height: u64,
    expiry_height: u64,
) -> Result<Vec<GovernedLeg>> {
    governed_legs(routes, authority_context_height, expiry_height)?
        .into_iter()
        .enumerate()
        .map(|(leg_ordinal, base)| {
            let mut policy_body = base.policy.body.clone();
            policy_body.policy_id = Hash::prehashed(fault_derivation_bytes(
                request,
                bundle_ordinal,
                leg_ordinal,
                b"audit-policy",
            ));
            let policy = PrivateSettlementAuditPolicyV1::new(policy_body)?;
            let governance = PrivateSettlementPoolGovernanceV1::from_restricted_mapping(
                base.route,
                PrivacyPoolIdV1::new(fault_derivation_bytes(
                    request,
                    bundle_ordinal,
                    leg_ordinal,
                    b"pool-id",
                )),
                base.governance.body.asset_definition_id.clone(),
                fault_derivation_bytes(request, bundle_ordinal, leg_ordinal, b"asset-binding-salt"),
                &policy,
                PrivateSettlementPoolGovernanceLifecycleV1 {
                    governance_revision: 1,
                    activation_height: authority_context_height,
                    retirement_height: Some(expiry_height + 1),
                },
            )?;
            Ok(GovernedLeg {
                route: base.route,
                policy,
                governance,
                auditor_signing: base.auditor_signing,
                auditor_encryption: base.auditor_encryption,
            })
        })
        .collect()
}

fn write_owner_only_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("owner-only file has no parent"))?;
    let metadata = fs::symlink_metadata(parent).wrap_err("inspect owner-only parent")?;
    ensure!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o777 == 0o700,
        "owner-only parent is unsafe"
    );
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| eyre!("owner-only file name is invalid"))?,
        std::process::id()
    ));
    let _ = fs::remove_file(&temporary);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .wrap_err("create owner-only temporary file")?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn read_owner_only_bounded(path: &Path) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path).wrap_err("inspect owner-only file")?;
    ensure!(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.permissions().mode() & 0o777 == 0o600
            && std::os::unix::fs::MetadataExt::uid(&before)
                == std::os::unix::fs::MetadataExt::uid(&fs::symlink_metadata(
                    path.parent()
                        .ok_or_else(|| eyre!("owner-only file lacks parent"))?,
                )?,)
            && usize::try_from(before.len()).is_ok_and(|len| len <= HARNESS_MAX_JSON_BYTES),
        "owner-only file is unsafe or oversized"
    );
    let mut file = File::open(path).wrap_err("open owner-only file")?;
    let opened = file.metadata()?;
    ensure!(
        std::os::unix::fs::MetadataExt::dev(&before)
            == std::os::unix::fs::MetadataExt::dev(&opened)
            && std::os::unix::fs::MetadataExt::ino(&before)
                == std::os::unix::fs::MetadataExt::ino(&opened),
        "owner-only file changed before open"
    );
    let mut bytes = Vec::new();
    file.by_ref()
        .take(u64::try_from(HARNESS_MAX_JSON_BYTES + 1).expect("bound fits u64"))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    ensure!(
        bytes.len() <= HARNESS_MAX_JSON_BYTES
            && opened.len() == after.len()
            && std::os::unix::fs::MetadataExt::mtime(&opened)
                == std::os::unix::fs::MetadataExt::mtime(&after)
            && std::os::unix::fs::MetadataExt::mtime_nsec(&opened)
                == std::os::unix::fs::MetadataExt::mtime_nsec(&after),
        "owner-only file changed while read"
    );
    Ok(bytes)
}

fn coordinator_client_config(client: &Client) -> Result<Vec<u8>> {
    let domain = iroha::data_model::domain::DomainId::try_new("default", "universal")?;
    let private_key = iroha_crypto::ExposedPrivateKey(client.key_pair.private_key().clone());
    let mut root = Table::new();
    root.insert(
        "chain".to_owned(),
        TomlValue::String(client.chain.to_string()),
    );
    root.insert(
        "network_id".to_owned(),
        TomlValue::String(client.network_id.to_string()),
    );
    root.insert(
        "torii_url".to_owned(),
        TomlValue::String(client.torii_url.to_string()),
    );
    root.insert(
        "torii_request_timeout_ms".to_owned(),
        TomlValue::Integer(i64::try_from(client.torii_request_timeout.as_millis())?),
    );
    let mut account = Table::new();
    account.insert("domain".to_owned(), TomlValue::String(domain.to_string()));
    account.insert(
        "public_key".to_owned(),
        TomlValue::String(client.key_pair.public_key().to_string()),
    );
    account.insert(
        "private_key".to_owned(),
        TomlValue::String(private_key.to_string()),
    );
    root.insert("account".to_owned(), TomlValue::Table(account));
    let mut transaction = Table::new();
    transaction.insert(
        "time_to_live_ms".to_owned(),
        TomlValue::Integer(i64::try_from(
            client
                .transaction_ttl
                .unwrap_or(Duration::from_secs(60))
                .as_millis(),
        )?),
    );
    transaction.insert(
        "status_timeout_ms".to_owned(),
        TomlValue::Integer(i64::try_from(
            client.transaction_status_timeout.as_millis(),
        )?),
    );
    transaction.insert(
        "nonce".to_owned(),
        TomlValue::Boolean(client.add_transaction_nonce),
    );
    root.insert("transaction".to_owned(), TomlValue::Table(transaction));
    Ok(toml::to_string(&root)?.into_bytes())
}

struct CoordinatorProcessV1 {
    root: tempfile::TempDir,
    child: Child,
    pid: u32,
    executable_sha256: String,
    revision: u64,
}

impl CoordinatorProcessV1 {
    fn start(client: &Client) -> Result<Self> {
        let root = tempfile::tempdir().wrap_err("create coordinator runtime root")?;
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))?;
        write_owner_only_atomic(
            &root.path().join(COORDINATOR_CONFIG_FILE),
            &coordinator_client_config(client)?,
        )?;
        let command = CoordinatorCommandV1 {
            format_version: 1,
            revision: 1,
            operation: "idle".to_owned(),
            committee_endpoints: Vec::new(),
            manifest: None,
            authority_catalog: Vec::new(),
            deltas: Vec::new(),
            barrier: None,
        };
        let command_bytes = canonical_harness_json_bytes(&command)?;
        write_owner_only_atomic(&root.path().join(COORDINATOR_COMMAND_FILE), &command_bytes)?;
        let executable = std::env::current_exe().wrap_err("resolve coordinator test binary")?;
        let executable_sha256 = sha256_regular_file(&executable)?;
        let child = Self::spawn_child(root.path(), &executable)?;
        let pid = child.id();
        let mut process = Self {
            root,
            child,
            pid,
            executable_sha256,
            revision: 1,
        };
        let (_bytes, acknowledgement) = process.wait_for_ack(&command_bytes)?;
        ensure!(
            acknowledgement.pid == pid && acknowledgement.operation == "idle",
            "coordinator helper did not acknowledge its live PID"
        );
        Ok(process)
    }

    fn spawn_child(root: &Path, executable: &Path) -> Result<Child> {
        Command::new(executable)
            .args([
                "--ignored",
                "--exact",
                "nexus::atomic_private_settlement_localnet::atomic_private_settlement_real_process_coordinator_helper",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(COORDINATOR_ROOT_ENV, root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .wrap_err("spawn coordinator helper process")
    }

    fn wait_for_ack(&mut self, command_bytes: &[u8]) -> Result<(Vec<u8>, CoordinatorAckV1)> {
        let expected = hex::encode(Sha256::digest(command_bytes));
        let path = self.root.path().join(COORDINATOR_ACK_FILE);
        let started = Instant::now();
        while started.elapsed() <= FINALITY_TIMEOUT {
            if let Some(status) = self.child.try_wait()? {
                return Err(eyre!(
                    "coordinator helper exited before acknowledgement: {status}"
                ));
            }
            if let Ok(bytes) = read_owner_only_bounded(&path) {
                let acknowledgement: CoordinatorAckV1 = norito::json::from_slice(&bytes)?;
                ensure!(
                    canonical_harness_json_bytes(&acknowledgement)? == bytes
                        && acknowledgement.format_version == 1
                        && acknowledgement.revision == self.revision
                        && acknowledgement.command_sha256 == expected
                        && acknowledgement.pid == self.pid,
                    "coordinator acknowledgement is substituted or non-canonical"
                );
                return Ok((bytes, acknowledgement));
            }
            thread::sleep(POLL_INTERVAL);
        }
        Err(eyre!("timed out waiting for coordinator acknowledgement"))
    }

    fn restart_with(
        &mut self,
        mut command: CoordinatorCommandV1,
    ) -> Result<(u32, u32, Vec<u8>, Vec<u8>, CoordinatorAckV1)> {
        let before_pid = self.pid;
        self.child.kill().wrap_err("kill coordinator helper")?;
        let status = self.child.wait().wrap_err("reap coordinator helper")?;
        ensure!(
            !status.success(),
            "killed coordinator helper exited successfully"
        );
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| eyre!("coordinator revision overflow"))?;
        command.revision = self.revision;
        let command_bytes = canonical_harness_json_bytes(&command)?;
        let ack_path = self.root.path().join(COORDINATOR_ACK_FILE);
        if ack_path.exists() {
            fs::remove_file(&ack_path)?;
        }
        write_owner_only_atomic(
            &self.root.path().join(COORDINATOR_COMMAND_FILE),
            &command_bytes,
        )?;
        let executable = std::env::current_exe().wrap_err("resolve coordinator test binary")?;
        self.child = Self::spawn_child(self.root.path(), &executable)?;
        self.pid = self.child.id();
        ensure!(
            self.pid != before_pid,
            "coordinator helper reused its killed PID"
        );
        let (ack_bytes, acknowledgement) = self.wait_for_ack(&command_bytes)?;
        Ok((
            before_pid,
            self.pid,
            command_bytes,
            ack_bytes,
            acknowledgement,
        ))
    }
}

impl Drop for CoordinatorProcessV1 {
    fn drop(&mut self) {
        let _ = write_owner_only_atomic(
            &self.root.path().join(COORDINATOR_SHUTDOWN_FILE),
            b"shutdown\n",
        );
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn run_coordinator_helper_process() -> Result<()> {
    let root = PathBuf::from(
        std::env::var(COORDINATOR_ROOT_ENV).wrap_err("missing coordinator runtime root")?,
    );
    let canonical = root
        .canonicalize()
        .wrap_err("resolve coordinator runtime root")?;
    ensure!(
        canonical == root,
        "coordinator runtime root is not canonical"
    );
    let metadata = fs::symlink_metadata(&root)?;
    ensure!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o777 == 0o700,
        "coordinator runtime root is unsafe"
    );
    let command_bytes = read_owner_only_bounded(&root.join(COORDINATOR_COMMAND_FILE))?;
    let command: CoordinatorCommandV1 = norito::json::from_slice(&command_bytes)?;
    ensure!(
        canonical_harness_json_bytes(&command)? == command_bytes
            && command.format_version == 1
            && command.revision > 0,
        "coordinator command is substituted or non-canonical"
    );
    let config_path = root.join(COORDINATOR_CONFIG_FILE);
    let config_bytes = read_owner_only_bounded(&config_path)?;
    let (config, _publication) =
        iroha::config::Config::load_bytes_with_musubi_publication(&config_path, &config_bytes)
            .map_err(|error| {
                eyre!("load stable explicit coordinator client configuration: {error:?}")
            })?;
    let client = Client::new(config);
    let endpoints = command
        .committee_endpoints
        .iter()
        .map(|committee| {
            committee
                .iter()
                .map(|endpoint| Url::parse(endpoint).wrap_err("parse coordinator endpoint"))
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    let (barrier, commit_certificates) = match command.operation.as_str() {
        "idle"
            if command.manifest.is_none()
                && command.authority_catalog.is_empty()
                && command.deltas.is_empty()
                && command.barrier.is_none()
                && endpoints.is_empty() =>
        {
            (None, Vec::new())
        }
        "recover_prepare" => {
            let manifest = command
                .manifest
                .as_ref()
                .ok_or_else(|| eyre!("recover_prepare lacks manifest"))?;
            let barrier = client.recover_or_prepare_private_settlement_bundle_v1(
                &endpoints,
                manifest,
                &command.authority_catalog,
                &command.deltas,
            )?;
            (Some(barrier), Vec::new())
        }
        "recover_prepare_commit" => {
            let manifest = command
                .manifest
                .as_ref()
                .ok_or_else(|| eyre!("recover_prepare_commit lacks manifest"))?;
            let barrier = client.recover_or_prepare_private_settlement_bundle_v1(
                &endpoints,
                manifest,
                &command.authority_catalog,
                &command.deltas,
            )?;
            let commits =
                client.recover_or_commit_private_settlement_bundle_v1(&endpoints, &barrier)?;
            (Some(barrier), commits)
        }
        "recover_commit" => {
            let barrier = command
                .barrier
                .as_ref()
                .ok_or_else(|| eyre!("recover_commit lacks barrier"))?;
            let commits =
                client.recover_or_commit_private_settlement_bundle_v1(&endpoints, barrier)?;
            (Some(barrier.clone()), commits)
        }
        _ => return Err(eyre!("coordinator command operation or fields are invalid")),
    };
    let acknowledgement = CoordinatorAckV1 {
        format_version: 1,
        revision: command.revision,
        command_sha256: hex::encode(Sha256::digest(&command_bytes)),
        pid: std::process::id(),
        operation: command.operation,
        barrier,
        commit_certificates,
    };
    write_owner_only_atomic(
        &root.join(COORDINATOR_ACK_FILE),
        &canonical_harness_json_bytes(&acknowledgement)?,
    )?;
    while !root.join(COORDINATOR_SHUTDOWN_FILE).exists() {
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

fn fault_state_identity(observation: &FaultStateObservationV1) -> Result<FaultStateIdentityV1> {
    Ok(FaultStateIdentityV1 {
        ledger_commitment: observation.ledger_commitment.clone(),
        staged_lock_commitment: observation.staged_lock_commitment.clone(),
        counts: norito::json::to_string(&observation.counts)
            .wrap_err("encode fault state count identity")?,
    })
}

fn fault_ledger_identity(observation: &FaultStateObservationV1) -> Result<FaultLedgerIdentityV1> {
    let mut counts = observation
        .counts
        .as_object()
        .ok_or_else(|| eyre!("fault state count vector is not an object"))?
        .clone();
    for field in [
        "staged_pool_heads",
        "staged_nullifiers",
        "staged_output_commitments",
        "staged_locks",
    ] {
        ensure!(counts.remove(field).is_some(), "fault state lacks {field}");
    }
    Ok(FaultLedgerIdentityV1 {
        ledger_commitment: observation.ledger_commitment.clone(),
        counts: norito::json::to_string(&HarnessJsonValue::Object(counts))
            .wrap_err("encode fault ledger count identity")?,
    })
}

fn capture_fault_state_observation(
    peer_index: usize,
    peer: &NetworkPeer,
) -> Result<FaultStateObservationV1> {
    let response = peer
        .client()
        .private_settlement_test_network_state_evidence_v1()
        .wrap_err_with(|| format!("query APS state evidence from validator #{peer_index}"))?;
    let response_bytes = canonical_harness_json_bytes(&response)?;
    let counts =
        norito::json::to_value(&response.counts).wrap_err("encode APS state evidence counts")?;
    Ok(FaultStateObservationV1 {
        peer_index,
        response_sha256: hex::encode(Sha256::digest(&response_bytes)),
        response_hex: hex::encode(response_bytes),
        height: response.height,
        commitment: response.commitment.to_string(),
        ledger_commitment: response.ledger_commitment.to_string(),
        staged_lock_commitment: response.staged_lock_commitment.to_string(),
        counts,
    })
}

fn capture_fault_state_snapshot(network: &Network, label: &str) -> Result<FaultStateSnapshotV1> {
    let validators = network
        .peers()
        .iter()
        .enumerate()
        .map(|(peer_index, peer)| capture_fault_state_observation(peer_index, peer))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        validators.len() == network.peers().len(),
        "fault state snapshot omitted a validator"
    );
    Ok(FaultStateSnapshotV1 {
        label: label.to_owned(),
        validators,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FaultContinuousObservationClassV1 {
    Baseline,
    Finalized,
}

fn classify_fault_continuous_observation(
    baseline: &FaultStateObservationV1,
    observation: &FaultStateObservationV1,
    participants: usize,
) -> Result<FaultContinuousObservationClassV1> {
    ensure!(
        baseline.peer_index == observation.peer_index,
        "continuous APS observation changed validator identity"
    );
    if fault_ledger_identity(baseline)? == fault_ledger_identity(observation)? {
        return Ok(FaultContinuousObservationClassV1::Baseline);
    }
    ensure!(
        baseline.ledger_commitment != observation.ledger_commitment,
        "continuous APS observation changed counts without changing the ledger commitment"
    );
    let expected_deltas = [
        ("roots", participants),
        ("nullifiers", participants * 2),
        ("commitments", participants * 3),
        ("encrypted_outputs", participants * 3),
        ("replay_markers", 1),
        ("receipts", 1),
    ];
    for (field, delta) in expected_deltas {
        let expected = fault_count(&baseline.counts, field)?
            .checked_add(u64::try_from(delta)?)
            .ok_or_else(|| eyre!("continuous APS `{field}` count overflow"))?;
        ensure!(
            fault_count(&observation.counts, field)? == expected,
            "validator #{} exposed a partial APS `{field}` count",
            observation.peer_index
        );
    }
    for field in ["governance", "pools", "abort_markers"] {
        ensure!(
            fault_count(&observation.counts, field)? == fault_count(&baseline.counts, field)?,
            "validator #{} changed APS `{field}` outside one complete finalization",
            observation.peer_index
        );
    }
    Ok(FaultContinuousObservationClassV1::Finalized)
}

struct FaultContinuousObservationAccumulatorV1 {
    peer_index: usize,
    check_count: u64,
    first_response_sha256: Option<String>,
    last_response_sha256: Option<String>,
    response_chain: Sha256,
    baseline_observations: u64,
    finalized_observations: u64,
}

impl FaultContinuousObservationAccumulatorV1 {
    fn new(peer_index: usize) -> Self {
        let mut response_chain = Sha256::new();
        response_chain.update(FAULT_CONTINUOUS_OBSERVATION_DOMAIN_V1);
        response_chain.update(
            u64::try_from(peer_index)
                .expect("peer index fits u64")
                .to_le_bytes(),
        );
        Self {
            peer_index,
            check_count: 0,
            first_response_sha256: None,
            last_response_sha256: None,
            response_chain,
            baseline_observations: 0,
            finalized_observations: 0,
        }
    }

    fn record(
        &mut self,
        baseline: &FaultStateObservationV1,
        observation: &FaultStateObservationV1,
        participants: usize,
    ) -> Result<()> {
        let class = classify_fault_continuous_observation(baseline, observation, participants)?;
        let digest = hex::decode(&observation.response_sha256)
            .wrap_err("decode continuous APS response digest")?;
        ensure!(
            digest.len() == Hash::LENGTH,
            "continuous APS response digest has the wrong length"
        );
        self.check_count = self
            .check_count
            .checked_add(1)
            .ok_or_else(|| eyre!("continuous APS observation count overflow"))?;
        self.first_response_sha256
            .get_or_insert_with(|| observation.response_sha256.clone());
        self.last_response_sha256 = Some(observation.response_sha256.clone());
        self.response_chain.update(&digest);
        match class {
            FaultContinuousObservationClassV1::Baseline => {
                self.baseline_observations = self
                    .baseline_observations
                    .checked_add(1)
                    .ok_or_else(|| eyre!("continuous APS baseline count overflow"))?;
            }
            FaultContinuousObservationClassV1::Finalized => {
                self.finalized_observations = self
                    .finalized_observations
                    .checked_add(1)
                    .ok_or_else(|| eyre!("continuous APS finalized count overflow"))?;
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<FaultContinuousObservationSummaryV1> {
        ensure!(
            self.check_count >= 3,
            "continuous APS observer did not record a live poll between its bound endpoints"
        );
        Ok(FaultContinuousObservationSummaryV1 {
            peer_index: self.peer_index,
            check_count: self.check_count,
            first_response_sha256: self
                .first_response_sha256
                .ok_or_else(|| eyre!("continuous APS observer lacks a first response"))?,
            last_response_sha256: self
                .last_response_sha256
                .ok_or_else(|| eyre!("continuous APS observer lacks a last response"))?,
            response_chain_sha256: hex::encode(self.response_chain.finalize()),
            baseline_observations: self.baseline_observations,
            finalized_observations: self.finalized_observations,
        })
    }
}

struct FaultContinuousObserverV1 {
    stop: Arc<AtomicBool>,
    accumulators: Arc<Mutex<Vec<FaultContinuousObservationAccumulatorV1>>>,
    failure: Arc<Mutex<Option<String>>>,
    baselines: Vec<FaultStateObservationV1>,
    participants: usize,
    handle: Option<thread::JoinHandle<()>>,
}

impl FaultContinuousObserverV1 {
    fn start(
        network: &Network,
        before: &FaultStateSnapshotV1,
        participants: usize,
    ) -> Result<Self> {
        ensure_fault_state_converged(before)?;
        ensure!(
            before.validators.len() == network.peers().len(),
            "continuous APS observer baseline omits validators"
        );
        let baselines = before.validators.clone();
        let mut initial = (0..baselines.len())
            .map(FaultContinuousObservationAccumulatorV1::new)
            .collect::<Vec<_>>();
        for (accumulator, observation) in initial.iter_mut().zip(&baselines) {
            accumulator.record(observation, observation, participants)?;
        }
        let accumulators = Arc::new(Mutex::new(initial));
        let failure = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let peers = network.peers().to_vec();
        let thread_baselines = baselines.clone();
        let thread_accumulators = Arc::clone(&accumulators);
        let thread_failure = Arc::clone(&failure);
        let thread_stop = Arc::clone(&stop);
        let handle = thread::Builder::new()
            .name("aps-fault-continuous-observer".to_owned())
            .spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    for (peer_index, peer) in peers.iter().enumerate() {
                        if thread_stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let Ok(observation) = capture_fault_state_observation(peer_index, peer)
                        else {
                            continue;
                        };
                        let result = thread_accumulators
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)[peer_index]
                            .record(&thread_baselines[peer_index], &observation, participants);
                        if let Err(error) = result {
                            *thread_failure
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                                Some(error.to_string());
                            thread_stop.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                    if !thread_stop.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(25));
                    }
                }
            })?;
        let observer = Self {
            stop,
            accumulators,
            failure,
            baselines,
            participants,
            handle: Some(handle),
        };
        observer.wait_for_initial_poll()?;
        Ok(observer)
    }

    fn wait_for_initial_poll(&self) -> Result<()> {
        let started = Instant::now();
        loop {
            if let Some(error) = self
                .failure
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .cloned()
            {
                return Err(eyre!("continuous APS observer rejected state: {error}"));
            }
            if self
                .accumulators
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .all(|accumulator| accumulator.check_count >= 2)
            {
                return Ok(());
            }
            ensure!(
                started.elapsed() <= FAULT_CONTROL_TIMEOUT,
                "continuous APS observer did not poll every validator before the trial"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn observe_snapshot(&self, snapshot: &FaultStateSnapshotV1) -> Result<()> {
        ensure!(
            snapshot.validators.len() == self.baselines.len(),
            "continuous APS snapshot omits validators"
        );
        let mut accumulators = self
            .accumulators
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for ((accumulator, baseline), observation) in accumulators
            .iter_mut()
            .zip(&self.baselines)
            .zip(&snapshot.validators)
        {
            accumulator.record(baseline, observation, self.participants)?;
        }
        Ok(())
    }

    fn stop_and_join(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| eyre!("continuous APS observer thread panicked"))?;
        }
        if let Some(error) = self
            .failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            return Err(eyre!("continuous APS observer rejected state: {error}"));
        }
        Ok(())
    }

    fn finish(
        mut self,
        after: &FaultStateSnapshotV1,
    ) -> Result<Vec<FaultContinuousObservationSummaryV1>> {
        self.stop_and_join()?;
        self.observe_snapshot(after)?;
        let accumulators = {
            let mut guard = self
                .accumulators
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *guard)
        };
        accumulators
            .into_iter()
            .map(FaultContinuousObservationAccumulatorV1::finish)
            .collect()
    }
}

impl Drop for FaultContinuousObserverV1 {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}

fn wait_for_converged_fault_state_snapshot(
    network: &Network,
    label: &str,
) -> Result<FaultStateSnapshotV1> {
    let started = Instant::now();
    let mut last = None;
    while started.elapsed() <= FINALITY_TIMEOUT {
        match capture_fault_state_snapshot(network, label) {
            Ok(snapshot) if ensure_fault_state_converged(&snapshot).is_ok() => return Ok(snapshot),
            Ok(snapshot) => {
                last = Some(format!(
                    "non-converged {}-validator snapshot",
                    snapshot.validators.len()
                ))
            }
            Err(error) => last = Some(error.to_string()),
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "timed out waiting for a coherent APS state snapshot `{label}`: {}",
        last.unwrap_or_else(|| "no state response".to_owned())
    ))
}

fn fault_count(value: &HarnessJsonValue, name: &str) -> Result<u64> {
    value
        .get(name)
        .and_then(HarnessJsonValue::as_u64)
        .ok_or_else(|| eyre!("APS evidence count `{name}` is absent"))
}

fn ensure_fault_state_finalized_once(
    before: &FaultStateSnapshotV1,
    after: &FaultStateSnapshotV1,
    participants: usize,
) -> Result<()> {
    ensure_fault_state_converged(before)?;
    ensure_fault_state_converged(after)?;
    let before = &before.validators[0];
    let after = &after.validators[0];
    ensure!(
        before.ledger_commitment != after.ledger_commitment
            && before.staged_lock_commitment == after.staged_lock_commitment,
        "finalized APS trial did not change exactly the global ledger"
    );
    let expected_deltas = [
        ("roots", participants),
        ("nullifiers", participants * 2),
        ("commitments", participants * 3),
        ("encrypted_outputs", participants * 3),
        ("replay_markers", 1),
        ("receipts", 1),
    ];
    for (field, delta) in expected_deltas {
        ensure!(
            fault_count(&after.counts, field)?
                == fault_count(&before.counts, field)? + u64::try_from(delta)?,
            "finalized APS trial has the wrong `{field}` count delta"
        );
    }
    for field in [
        "governance",
        "pools",
        "abort_markers",
        "staged_pool_heads",
        "staged_nullifiers",
        "staged_output_commitments",
        "staged_locks",
    ] {
        ensure!(
            fault_count(&after.counts, field)? == fault_count(&before.counts, field)?,
            "finalized APS trial unexpectedly changed `{field}`"
        );
    }
    Ok(())
}

fn ensure_fault_state_reverted(
    before: &FaultStateSnapshotV1,
    after: &FaultStateSnapshotV1,
) -> Result<()> {
    ensure!(
        before.validators.len() == after.validators.len() && !before.validators.is_empty(),
        "fault state snapshots have different validator inventories"
    );
    for (before, after) in before.validators.iter().zip(&after.validators) {
        ensure!(
            before.peer_index == after.peer_index
                && fault_state_identity(before)? == fault_state_identity(after)?,
            "validator #{} changed APS state after a nonfinalized trial",
            before.peer_index
        );
    }
    Ok(())
}

fn ensure_fault_state_converged(snapshot: &FaultStateSnapshotV1) -> Result<()> {
    let first = snapshot
        .validators
        .first()
        .ok_or_else(|| eyre!("fault state convergence snapshot is empty"))?;
    let expected = fault_state_identity(first)?;
    for observation in &snapshot.validators {
        ensure!(
            fault_state_identity(observation)? == expected,
            "validators did not converge on one APS state commitment/count vector"
        );
    }
    Ok(())
}

fn ensure_fault_ledger_unchanged_before_finality(
    before: &FaultStateSnapshotV1,
    nonfinalized: &FaultStateSnapshotV1,
) -> Result<()> {
    ensure!(
        before.validators.len() == nonfinalized.validators.len() && !before.validators.is_empty(),
        "fault state snapshots have different validator inventories"
    );
    let expected = fault_ledger_identity(&before.validators[0])?;
    for observation in &before.validators {
        ensure!(
            fault_ledger_identity(observation)? == expected,
            "validators lack a coherent APS ledger before the trial"
        );
    }
    for observation in &nonfinalized.validators {
        ensure!(
            fault_ledger_identity(observation)? == expected,
            "validator #{} exposed a global APS map change before finality",
            observation.peer_index
        );
    }
    Ok(())
}

fn verify_signed_rs16_finality(network: &Network, finalized_height: u64) -> Result<u64> {
    let height = NonZeroU64::new(finalized_height)
        .ok_or_else(|| eyre!("finalized receipt height is zero"))?;
    let expected_layout = SumeragiV2GenesisContextParameters::recommended().da_layout;
    let mut observations = 0_u64;
    for peer in network.peers() {
        let (proof, block_hash) = peer
            .client()
            .get_bridge_finality_anchor(height, network.network_id())
            .wrap_err_with(|| format!("fetch signed finality proof from {}", peer.id()))?;
        let artifact = &proof.finality_artifact;
        ensure!(
            proof.block_header.hash() == block_hash
                && artifact.height_context.roster.len() == VALIDATORS_PER_LANE
                && artifact.height_context.quorum.min_signers == 3
                && artifact.commit_qc.signers.len() == 3
                && artifact.height_context.da_layout == expected_layout,
            "peer {} did not return a signed 3-of-4 RS16 finality artifact",
            peer.id()
        );
        observations += 1;
    }
    Ok(observations)
}

fn prepare_fault_bundle(
    request: &RealProcessFaultRequestV1,
    bundle_ordinal: usize,
    sponsor: &Client,
    network: &Network,
    routes: &[PrivateSettlementRouteV1],
    committees: &[CommitteeEndpoints],
) -> Result<FaultPreparedBundleV1> {
    let current_height = sponsor.get_privacy_capabilities()?.committed_height;
    let authority_context_height = current_height
        .checked_add(1)
        .ok_or_else(|| eyre!("fault authority height overflow"))?;
    let expiry_height = authority_context_height
        .checked_add(FAULT_BUNDLE_EXPIRY_BLOCKS)
        .ok_or_else(|| eyre!("fault expiry height overflow"))?;
    let governed = fault_governed_legs(
        request,
        bundle_ordinal,
        routes,
        authority_context_height,
        expiry_height,
    )?;
    let manifest = proof_manifest(
        network.network_id(),
        authority_context_height,
        expiry_height,
        &governed,
    )?;
    let prepared = governed
        .into_iter()
        .zip(committees)
        .enumerate()
        .map(|(ordinal, (leg, committee))| {
            prepare_leg(ordinal, leg, &manifest, committee.authority.digest()?)
        })
        .collect::<Result<Vec<_>>>()?;
    let activations = prepared
        .iter()
        .map(|leg| {
            ActivatePrivateSettlementPoolV1::from_restricted(
                &leg.governed.governance,
                leg.initial_commitments.to_vec(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let activation =
        sponsor.build_transaction_from_items(activations, no_fee(), Metadata::default());
    sponsor
        .submit_transaction_blocking(&activation)
        .wrap_err("activate fault-campaign private pools")?;
    ensure!(
        sponsor.get_privacy_capabilities()?.committed_height == authority_context_height,
        "fault pool activation did not land at the bound authority height"
    );
    let materials = provisional_materials(manifest.clone(), &prepared, committees)?;
    let authorities = committees
        .iter()
        .map(|committee| committee.authority.clone())
        .collect();
    let deltas = prepared.iter().map(|leg| leg.delta.clone()).collect();
    Ok(FaultPreparedBundleV1 {
        manifest,
        prepared,
        materials,
        authorities,
        deltas,
    })
}

fn certify_and_upload_fault_bundle(
    sponsor: &Client,
    bundle: &mut FaultPreparedBundleV1,
    committees: &[CommitteeEndpoints],
) -> Result<()> {
    let certificates = bundle
        .materials
        .iter()
        .zip(committees)
        .map(|(material, committee)| {
            sponsor.certify_private_settlement_leg_availability_v1(&committee.endpoints, material)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut final_manifest = bundle.materials[0].manifest.clone();
    for (ordinal, certificate) in certificates.iter().enumerate() {
        final_manifest.legs[ordinal].availability_certificate_digest = certificate.digest()?;
    }
    final_manifest.validate()?;
    for (ordinal, ((material, certificate), committee)) in bundle
        .materials
        .iter()
        .zip(&certificates)
        .zip(committees)
        .enumerate()
    {
        let upload = PrivateSettlementLegUploadRequestV1 {
            manifest: final_manifest.clone(),
            audit_policy: material.audit_policy.clone(),
            committee_authority: material.committee_authority.clone(),
            payload: material.payload_with_certificate(certificate.clone()),
        };
        for endpoint in &committee.endpoints {
            let response = sponsor.upload_private_settlement_leg_to_v1(endpoint, &upload)?;
            ensure!(
                usize::from(response.leg_ordinal) == ordinal,
                "fault upload ordinal was substituted"
            );
        }
    }
    bundle.manifest = final_manifest;
    Ok(())
}

fn audit_fault_bundle(
    sponsor: &Client,
    bundle: &FaultPreparedBundleV1,
    committees: &[CommitteeEndpoints],
) -> Result<()> {
    for (ordinal, (leg, committee)) in bundle.prepared.iter().zip(committees).enumerate() {
        let fetched = sponsor.private_settlement_auditor_capsule_v1(
            bundle.manifest.legs[ordinal].payload_digest,
            &leg.governed.auditor_signing,
        )?;
        let view = PrivateSettlementAuditorSidecarViewV1 {
            manifest: fetched.manifest,
            policy: fetched.audit_policy,
            authority: fetched.committee_authority,
            statement: fetched.statement,
            delta: fetched.delta,
            audit_capsule: fetched.audit_capsule,
            availability: fetched.availability,
            lifecycle: PrivateSettlementSidecarLifecycleV1::Collecting,
        };
        let auditor_id = AccountId::new(leg.governed.auditor_signing.public_key().clone());
        let approval = approve_private_settlement_leg_v1(
            &view,
            &leg.governed.governance,
            bundle.manifest.authority_context_height,
            &auditor_id,
            leg.governed.auditor_encryption.secret(),
            &leg.governed.auditor_signing,
            &approve_all_audit_material,
        )?;
        for endpoint in &committee.endpoints {
            let mut endpoint_client = sponsor.clone();
            endpoint_client.torii_url = endpoint.clone();
            let response = endpoint_client.submit_private_settlement_audit_approval_v1(
                bundle.manifest.legs[ordinal].payload_digest,
                &leg.governed.auditor_signing,
                &PrivateSettlementAuditApprovalRequestV1 {
                    approval: approval.clone(),
                },
            )?;
            ensure!(
                response.lifecycle == PrivateSettlementLifecycleDtoV1::Audited,
                "fault audit approval was not durable"
            );
        }
    }
    Ok(())
}

fn fault_endpoint_matrix(committees: &[CommitteeEndpoints]) -> Vec<Vec<Url>> {
    committees
        .iter()
        .map(|committee| committee.endpoints.clone())
        .collect()
}

fn finalize_fault_bundle(
    sponsor: &Client,
    network: &Network,
    bundle: &FaultPreparedBundleV1,
    barrier: iroha::data_model::nexus::PrivateSettlementPrepareBarrierV1,
    commits: Vec<iroha::data_model::nexus::PrivateSettlementPhaseCertificateV1>,
) -> Result<iroha::data_model::nexus::PrivateSettlementReceiptV1> {
    let legs = bundle
        .deltas
        .iter()
        .cloned()
        .zip(barrier.prepare_certificates.iter().cloned())
        .zip(commits)
        .map(|((delta, prepare), commit)| PrivateSettlementLegReceiptV1 {
            delta,
            prepare,
            commit,
        })
        .collect();
    let carrier = FinalizeAtomicPrivateSettlementV1::new(PrivateSettlementCommitBundleV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest: bundle.manifest.clone(),
        authority_catalog: bundle.authorities.clone(),
        legs,
    });
    let transaction = sponsor.build_transaction(
        [InstructionBox::from(carrier)],
        bundle.manifest.public_fee_intent.clone(),
        Metadata::default(),
    );
    sponsor.submit_private_settlement_bundle_v1(&PrivateSettlementBundleSubmitRequestV1 {
        transaction,
    })?;
    wait_for_identical_receipt(network, bundle.manifest.bundle_id)
}

fn route_control_type(phase: PrivateSettlementRouteControlPhase) -> &'static str {
    match phase {
        PrivateSettlementRouteControlPhase::RestrictedDa => "restricted_da",
        PrivateSettlementRouteControlPhase::Prepare => "prepare",
        PrivateSettlementRouteControlPhase::Commit => "commit",
    }
}

fn exercise_route_loss<F>(
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    peer_index: usize,
    phase: PrivateSettlementRouteControlPhase,
    bundle_id: Hash,
    seed: u64,
    loss_percent: u8,
    mut request: F,
) -> Result<(Vec<FaultControlOccurrenceV1>, FaultStateSnapshotV1)>
where
    F: FnMut() -> Result<()>,
{
    let peer = &network.peers()[peer_index];
    let control = peer
        .consensus_message_control()
        .ok_or_else(|| eyre!("fault peer lacks APS route control"))?;
    let drop_first = FAULT_ROUTE_MATCHES
        .checked_mul(u64::from(loss_percent))
        .and_then(|value| value.checked_div(100))
        .ok_or_else(|| eyre!("fault loss ratio overflow"))?;
    ensure!(
        drop_first * 100 == FAULT_ROUTE_MATCHES * u64::from(loss_percent),
        "fault loss ratio is not exactly representable"
    );
    let command = control.arm_private_settlement_route_control(
        phase,
        PrivateSettlementRouteControlAction::Loss,
        *bundle_id.as_ref(),
        seed,
        drop_first,
        FAULT_ROUTE_MATCHES,
    )?;
    let mut rejected = 0_u64;
    for _ in 0..FAULT_ROUTE_MATCHES {
        if request().is_err() {
            rejected += 1;
        }
    }
    ensure!(
        rejected == drop_first,
        "APS route control did not reject the exact configured loss count"
    );
    let (loss_ack_bytes, loss_ack) = runtime.block_on(
        control.wait_for_private_settlement_route_control(&command, FAULT_CONTROL_TIMEOUT),
    )?;
    ensure!(
        loss_ack.matched == FAULT_ROUTE_MATCHES
            && loss_ack.dropped == drop_first
            && loss_ack.passed == FAULT_ROUTE_MATCHES - drop_first
            && loss_ack.held == 0
            && loss_ack.released == 0,
        "APS loss acknowledgement counters differ from the in-flight trial"
    );
    let loss_occurrence = fault_control_occurrence(
        route_control_type(phase),
        Some(peer_index),
        command.canonical_bytes,
        loss_ack_bytes,
        None,
        None,
    )?;

    let healing = control.arm_private_settlement_route_control(
        phase,
        PrivateSettlementRouteControlAction::Pass,
        *bundle_id.as_ref(),
        seed,
        0,
        0,
    )?;
    request().wrap_err("APS route healing request did not pass")?;
    let (healing_ack_bytes, healing_ack) = runtime.block_on(
        control.wait_for_private_settlement_route_control(&healing, FAULT_CONTROL_TIMEOUT),
    )?;
    ensure!(
        healing_ack.matched == 1
            && healing_ack.passed == 1
            && healing_ack.dropped == 0
            && healing_ack.held == 0
            && healing_ack.released == 0,
        "APS route healing acknowledgement is not exact"
    );
    let healing_occurrence = fault_control_occurrence(
        route_control_type(phase),
        Some(peer_index),
        healing.canonical_bytes,
        healing_ack_bytes,
        None,
        None,
    )?;
    let nonfinalized = capture_fault_state_snapshot(network, "nonfinalized")?;
    Ok((vec![loss_occurrence, healing_occurrence], nonfinalized))
}

fn exercise_route_hold<F>(
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    peer_index: usize,
    phase: PrivateSettlementRouteControlPhase,
    bundle_id: Hash,
    seed: u64,
    request: F,
) -> Result<(Vec<FaultControlOccurrenceV1>, FaultStateSnapshotV1)>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    let peer = &network.peers()[peer_index];
    let control = peer
        .consensus_message_control()
        .ok_or_else(|| eyre!("fault peer lacks APS route control"))?;
    let hold = control.arm_private_settlement_route_control(
        phase,
        PrivateSettlementRouteControlAction::Hold,
        *bundle_id.as_ref(),
        seed,
        0,
        1,
    )?;
    let request_thread = thread::Builder::new()
        .name(format!("aps-fault-{}-hold", route_control_type(phase)))
        .spawn(request)?;
    let (hold_ack_bytes, hold_ack) = runtime.block_on(
        control.wait_for_private_settlement_route_control(&hold, FAULT_CONTROL_TIMEOUT),
    )?;
    ensure!(
        hold_ack.matched == 1
            && hold_ack.held == 1
            && hold_ack.passed == 0
            && hold_ack.dropped == 0
            && hold_ack.released == 0,
        "APS Hold acknowledgement is not exact"
    );
    let nonfinalized = capture_fault_state_snapshot(network, "nonfinalized")?;
    let pass = control.arm_private_settlement_route_control(
        phase,
        PrivateSettlementRouteControlAction::Pass,
        *bundle_id.as_ref(),
        seed,
        0,
        0,
    )?;
    request_thread
        .join()
        .map_err(|_| eyre!("APS held request thread panicked"))??;
    let (pass_ack_bytes, pass_ack) = runtime.block_on(
        control.wait_for_private_settlement_route_control(&pass, FAULT_CONTROL_TIMEOUT),
    )?;
    ensure!(
        pass_ack.predecessor_command_sha256.as_deref() == Some(hold.sha256.as_str())
            && pass_ack.matched == 1
            && pass_ack.held == 1
            && pass_ack.released == 1,
        "APS Hold-to-Pass acknowledgement lost predecessor evidence"
    );
    Ok((
        vec![
            fault_control_occurrence(
                route_control_type(phase),
                Some(peer_index),
                hold.canonical_bytes,
                hold_ack_bytes,
                None,
                None,
            )?,
            fault_control_occurrence(
                route_control_type(phase),
                Some(peer_index),
                pass.canonical_bytes,
                pass_ack_bytes,
                None,
                None,
            )?,
        ],
        nonfinalized,
    ))
}

fn stop_peer_for_quorum_progress(
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    peer_index: usize,
    revision: u64,
) -> Result<FaultStoppedPeerV1> {
    let peer = network.peers()[peer_index].clone();
    let before_pid = runtime
        .block_on(peer.process_id())
        .ok_or_else(|| eyre!("quorum-unavailability target has no live PID"))?;
    let command = FaultRestartCommandV1 {
        format_version: 1,
        revision,
        operation: "stop_validator_for_quorum_progress".to_owned(),
        peer_index,
        before_pid,
    };
    let command_bytes = canonical_harness_json_bytes(&command)?;
    ensure!(
        runtime.block_on(peer.shutdown_if_started())
            && runtime.block_on(peer.process_id()).is_none(),
        "quorum-unavailability target did not stop"
    );
    Ok(FaultStoppedPeerV1 {
        peer,
        peer_index,
        before_pid,
        revision,
        command_bytes,
    })
}

fn restart_quorum_progress_peer(
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    stopped: FaultStoppedPeerV1,
) -> Result<FaultControlOccurrenceV1> {
    let config_layers = network.config_layers().collect::<Vec<_>>();
    runtime.block_on(stopped.peer.start_checked(config_layers.iter(), None))?;
    let after_pid = runtime
        .block_on(stopped.peer.process_id())
        .ok_or_else(|| eyre!("quorum-progress restart has no live PID"))?;
    ensure!(
        after_pid != stopped.before_pid && stopped.peer.client().get_status().is_ok(),
        "quorum-progress restart did not produce a healthy new process"
    );
    let acknowledgement = FaultRestartAckV1 {
        format_version: 1,
        revision: stopped.revision,
        command_sha256: sha256_hex(&stopped.command_bytes),
        operation: "validator_restarted_after_quorum_progress".to_owned(),
        peer_index: stopped.peer_index,
        before_pid: stopped.before_pid,
        after_pid,
        health_observed: true,
    };
    fault_control_occurrence(
        "validator_restart",
        Some(stopped.peer_index),
        stopped.command_bytes,
        canonical_harness_json_bytes(&acknowledgement)?,
        Some(stopped.before_pid),
        Some(after_pid),
    )
}

fn restart_peer_with_evidence(
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    peer_index: usize,
    revision: u64,
    control_type: &str,
) -> Result<FaultControlOccurrenceV1> {
    let peer = network.peers()[peer_index].clone();
    let before_pid = runtime
        .block_on(peer.process_id())
        .ok_or_else(|| eyre!("restart target has no live PID"))?;
    let command = FaultRestartCommandV1 {
        format_version: 1,
        revision,
        operation: "restart_validator".to_owned(),
        peer_index,
        before_pid,
    };
    let command_bytes = canonical_harness_json_bytes(&command)?;
    let config_layers = network.config_layers().collect::<Vec<_>>();
    ensure!(
        runtime.block_on(peer.shutdown_if_started()),
        "restart target was not running"
    );
    runtime.block_on(peer.start_checked(config_layers.iter(), None))?;
    let after_pid = runtime
        .block_on(peer.process_id())
        .ok_or_else(|| eyre!("restarted target has no live PID"))?;
    ensure!(
        after_pid != before_pid && peer.client().get_status().is_ok(),
        "validator restart did not produce a healthy new process"
    );
    let acknowledgement = FaultRestartAckV1 {
        format_version: 1,
        revision,
        command_sha256: sha256_hex(&command_bytes),
        operation: command.operation,
        peer_index,
        before_pid,
        after_pid,
        health_observed: true,
    };
    fault_control_occurrence(
        control_type,
        Some(peer_index),
        command_bytes,
        canonical_harness_json_bytes(&acknowledgement)?,
        Some(before_pid),
        Some(after_pid),
    )
}

fn aggregate_fault_phase_votes(
    votes: &[iroha::data_model::nexus::PrivateSettlementPhaseVoteV1],
    signer_subset: [usize; 3],
    authority_catalog_index: u8,
) -> Result<iroha::data_model::nexus::PrivateSettlementPhaseCertificateV1> {
    ensure!(
        votes.len() == VALIDATORS_PER_LANE,
        "fault vote roster is incomplete"
    );
    let body = votes[signer_subset[0]].body.clone();
    ensure!(
        signer_subset.iter().all(|index| votes[*index].body == body),
        "fault votes do not certify one body"
    );
    let signatures = signer_subset
        .iter()
        .map(|index| votes[*index].signature.as_slice())
        .collect::<Vec<_>>();
    let signers_bitmap = signer_subset
        .iter()
        .fold(0_u8, |bitmap, index| bitmap | (1_u8 << index));
    Ok(
        iroha::data_model::nexus::PrivateSettlementPhaseCertificateV1 {
            body,
            authority_catalog_index,
            signers_bitmap,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&signatures)?,
        },
    )
}

fn prepare_fault_bundle_with_normalization(
    sponsor: &Client,
    bundle: &FaultPreparedBundleV1,
    committees: &[CommitteeEndpoints],
) -> Result<(
    iroha::data_model::nexus::PrivateSettlementPrepareBarrierV1,
    FaultPrepareQcNormalizationV1,
)> {
    let mut first_certificates = Vec::with_capacity(committees.len());
    let mut alternate_first = None;
    for (ordinal, committee) in committees.iter().enumerate() {
        let payload_digest = bundle.manifest.legs[ordinal].payload_digest;
        let votes = committee
            .endpoints
            .iter()
            .map(|endpoint| {
                sponsor
                    .request_private_settlement_prepare_vote_v1(
                        endpoint,
                        &bundle.manifest,
                        payload_digest,
                        &committee.authority,
                    )
                    .map(|response| response.vote)
            })
            .collect::<Result<Vec<_>>>()?;
        let first = aggregate_fault_phase_votes(
            &votes,
            [0, 1, 2],
            u8::try_from(ordinal).expect("leg ordinal fits u8"),
        )?;
        if ordinal == 0 {
            alternate_first = Some(aggregate_fault_phase_votes(&votes, [0, 1, 3], 0)?);
        }
        for endpoint in &committee.endpoints {
            sponsor.persist_private_settlement_phase_certificate_v1(
                endpoint,
                &bundle.manifest,
                payload_digest,
                &first,
            )?;
        }
        first_certificates.push(first);
    }
    let alternate_first =
        alternate_first.ok_or_else(|| eyre!("fault normalization lacks leg 0"))?;
    let first_barrier = Client::build_private_settlement_prepare_barrier_v1(
        bundle.manifest.clone(),
        bundle.authorities.clone(),
        bundle.deltas.clone(),
        first_certificates.clone(),
    )?;
    let mut second_certificates = first_certificates.clone();
    second_certificates[0] = alternate_first.clone();
    let second_barrier = Client::build_private_settlement_prepare_barrier_v1(
        bundle.manifest.clone(),
        bundle.authorities.clone(),
        bundle.deltas.clone(),
        second_certificates,
    )?;
    ensure!(
        first_barrier.prepared_bundle_digest == second_barrier.prepared_bundle_digest,
        "quorum-equivalent Prepare QCs did not normalize to one bundle digest"
    );

    let first_qc = &first_certificates[0];
    let mut changed_body = alternate_first.clone();
    changed_body.body.bundle_id = Hash::prehashed([0xa5; Hash::LENGTH]);
    let mut changed_certificates = first_certificates.clone();
    changed_certificates[0] = changed_body;
    let changed_body_rejected = Client::build_private_settlement_prepare_barrier_v1(
        bundle.manifest.clone(),
        bundle.authorities.clone(),
        bundle.deltas.clone(),
        changed_certificates,
    )
    .is_err();
    let mut changed_index = alternate_first.clone();
    changed_index.authority_catalog_index = 1;
    let mut changed_index_certificates = first_certificates.clone();
    changed_index_certificates[0] = changed_index;
    let authority_index_binding_verified = Client::build_private_settlement_prepare_barrier_v1(
        bundle.manifest.clone(),
        bundle.authorities.clone(),
        bundle.deltas.clone(),
        changed_index_certificates,
    )
    .is_err();
    let mut changed_signed_body = alternate_first.clone();
    changed_signed_body.body.delta_digest = Hash::prehashed([0x5a; Hash::LENGTH]);
    let mut changed_signed_certificates = first_certificates.clone();
    changed_signed_certificates[0] = changed_signed_body;
    let signed_body_binding_verified = Client::build_private_settlement_prepare_barrier_v1(
        bundle.manifest.clone(),
        bundle.authorities.clone(),
        bundle.deltas.clone(),
        changed_signed_certificates,
    )
    .is_err();
    ensure!(
        changed_body_rejected && authority_index_binding_verified && signed_body_binding_verified,
        "Prepare QC negative binding probes were accepted"
    );
    let normalized_sha = sha256_hex(first_barrier.prepared_bundle_digest.as_ref());
    let normalization = FaultPrepareQcNormalizationV1 {
        first_signer_subset: vec![0, 1, 2],
        second_signer_subset: vec![0, 1, 3],
        certified_body_sha256: sha256_hex(&norito::encode_canonical(&first_qc.body)?),
        first_qc_sha256: sha256_hex(&norito::encode_canonical(first_qc)?),
        second_qc_sha256: sha256_hex(&norito::encode_canonical(&alternate_first)?),
        first_normalized_barrier_sha256: normalized_sha.clone(),
        second_normalized_barrier_sha256: normalized_sha,
        equivalent_subsets_accepted: true,
        changed_body_rejected,
        authority_index_binding_verified,
        signed_body_binding_verified,
    };
    Ok((first_barrier, normalization))
}

fn build_fault_carrier_submit(
    sponsor: &Client,
    bundle: &FaultPreparedBundleV1,
    barrier: &iroha::data_model::nexus::PrivateSettlementPrepareBarrierV1,
    commits: &[iroha::data_model::nexus::PrivateSettlementPhaseCertificateV1],
) -> PrivateSettlementBundleSubmitRequestV1 {
    let legs = bundle
        .deltas
        .iter()
        .cloned()
        .zip(barrier.prepare_certificates.iter().cloned())
        .zip(commits.iter().cloned())
        .map(|((delta, prepare), commit)| PrivateSettlementLegReceiptV1 {
            delta,
            prepare,
            commit,
        })
        .collect();
    let carrier = FinalizeAtomicPrivateSettlementV1::new(PrivateSettlementCommitBundleV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest: bundle.manifest.clone(),
        authority_catalog: bundle.authorities.clone(),
        legs,
    });
    PrivateSettlementBundleSubmitRequestV1 {
        transaction: sponsor.build_transaction(
            [InstructionBox::from(carrier)],
            bundle.manifest.public_fee_intent.clone(),
            Metadata::default(),
        ),
    }
}

fn verify_invalid_leg_carrier_is_state_byte_identical(
    sponsor: &Client,
    network: &Network,
    bundle: &FaultPreparedBundleV1,
    barrier: &iroha::data_model::nexus::PrivateSettlementPrepareBarrierV1,
    commits: &[iroha::data_model::nexus::PrivateSettlementPhaseCertificateV1],
) -> Result<()> {
    let before = wait_for_converged_fault_state_snapshot(network, "invalid-leg-before")?;
    let mut invalid_deltas = bundle.deltas.clone();
    let first = invalid_deltas
        .first_mut()
        .ok_or_else(|| eyre!("invalid-leg probe lacks a participant delta"))?;
    first.new_epoch = first
        .new_epoch
        .checked_add(1)
        .ok_or_else(|| eyre!("invalid-leg epoch probe overflow"))?;
    let legs = invalid_deltas
        .into_iter()
        .zip(barrier.prepare_certificates.iter().cloned())
        .zip(commits.iter().cloned())
        .map(|((delta, prepare), commit)| PrivateSettlementLegReceiptV1 {
            delta,
            prepare,
            commit,
        })
        .collect();
    let carrier = FinalizeAtomicPrivateSettlementV1::new(PrivateSettlementCommitBundleV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest: bundle.manifest.clone(),
        authority_catalog: bundle.authorities.clone(),
        legs,
    });
    let request = PrivateSettlementBundleSubmitRequestV1 {
        transaction: sponsor.build_transaction(
            [InstructionBox::from(carrier)],
            bundle.manifest.public_fee_intent.clone(),
            Metadata::default(),
        ),
    };
    ensure!(
        sponsor.submit_private_settlement_bundle_v1(&request).is_err(),
        "global carrier accepted an invalid private-settlement leg delta"
    );
    let after = wait_for_converged_fault_state_snapshot(network, "invalid-leg-after")?;
    ensure!(
        before.validators.len() == after.validators.len(),
        "invalid-leg probe changed the validator evidence inventory"
    );
    for (before_peer, after_peer) in before.validators.iter().zip(&after.validators) {
        ensure!(
            before_peer.peer_index == after_peer.peer_index
                && fault_state_identity(before_peer)? == fault_state_identity(after_peer)?,
            "invalid-leg carrier changed APS state on validator #{}",
            before_peer.peer_index
        );
    }
    Ok(())
}

fn carrier_hold_rules(
    receiver_index: usize,
    global_peer_ids: &[PeerId],
    height: u64,
) -> Vec<ConsensusMessageControlRule> {
    global_peer_ids
        .iter()
        .enumerate()
        .filter(|(sender_index, _)| *sender_index != receiver_index)
        .flat_map(|(_, sender)| {
            (0..16).map(move |view| {
                ConsensusMessageControlRule::exact(
                    sender.clone(),
                    ConsensusMessageControlKind::Proposal,
                    height,
                    view,
                    ConsensusMessageControlAction::Hold,
                )
            })
        })
        .collect()
}

fn exercise_consensus_carrier_hold(
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    sponsor: &Client,
    submit: PrivateSettlementBundleSubmitRequestV1,
) -> Result<(Vec<FaultControlOccurrenceV1>, FaultStateSnapshotV1)> {
    let height = sponsor
        .get_status()?
        .blocks
        .checked_add(1)
        .ok_or_else(|| eyre!("carrier control height overflow"))?;
    let global_peer_ids = network.peers()[0..VALIDATORS_PER_LANE]
        .iter()
        .map(NetworkPeer::id)
        .collect::<Vec<_>>();
    for receiver_index in 0..VALIDATORS_PER_LANE {
        let control = network.peers()[receiver_index]
            .consensus_message_control()
            .ok_or_else(|| eyre!("global peer lacks consensus carrier control"))?;
        runtime.block_on(control.apply(
            &carrier_hold_rules(receiver_index, &global_peer_ids, height),
            &[],
            256,
            FAULT_CONTROL_TIMEOUT,
        ))?;
    }
    let submitter = sponsor.clone();
    let submit_thread = thread::Builder::new()
        .name("aps-fault-carrier-submit".to_owned())
        .spawn(move || {
            submitter
                .submit_private_settlement_bundle_v1(&submit)
                .map(|_| ())
        })?;
    let started = Instant::now();
    let hold_evidence = loop {
        let evidence = (0..VALIDATORS_PER_LANE)
            .map(|index| {
                network.peers()[index]
                    .consensus_message_control()
                    .ok_or_else(|| eyre!("global peer lacks consensus carrier control"))?
                    .read_current_evidence()
            })
            .collect::<Result<Vec<_>>>()?;
        if evidence
            .iter()
            .all(|item| !item.acknowledgement.held.is_empty())
        {
            break evidence;
        }
        ensure!(
            started.elapsed() <= FAULT_CONTROL_TIMEOUT,
            "carrier controls did not durably hold an authenticated proposal"
        );
        thread::sleep(POLL_INTERVAL);
    };
    let nonfinalized = capture_fault_state_snapshot(network, "nonfinalized")?;
    let mut controls = Vec::new();
    for (peer_index, evidence) in hold_evidence.into_iter().enumerate() {
        controls.push(fault_control_occurrence(
            "consensus_carrier",
            Some(peer_index),
            evidence.command_bytes,
            evidence.acknowledgement_bytes,
            None,
            None,
        )?);
    }
    for peer_index in 0..VALIDATORS_PER_LANE {
        let control = network.peers()[peer_index]
            .consensus_message_control()
            .ok_or_else(|| eyre!("global peer lacks consensus carrier control"))?;
        runtime.block_on(control.heal_and_release_all(FAULT_CONTROL_TIMEOUT))?;
        let evidence = control.read_current_evidence()?;
        controls.push(fault_control_occurrence(
            "consensus_carrier",
            Some(peer_index),
            evidence.command_bytes,
            evidence.acknowledgement_bytes,
            None,
            None,
        )?);
    }
    submit_thread
        .join()
        .map_err(|_| eyre!("carrier submit thread panicked"))??;
    Ok((controls, nonfinalized))
}

fn restart_ack_occurrence(
    peer_index: usize,
    revision: u64,
    control_type: &str,
    before_pid: u32,
    after_pid: u32,
) -> Result<FaultControlOccurrenceV1> {
    let command = FaultRestartCommandV1 {
        format_version: 1,
        revision,
        operation: "recover_crashed_validator".to_owned(),
        peer_index,
        before_pid,
    };
    let command_bytes = canonical_harness_json_bytes(&command)?;
    let acknowledgement = FaultRestartAckV1 {
        format_version: 1,
        revision,
        command_sha256: sha256_hex(&command_bytes),
        operation: command.operation,
        peer_index,
        before_pid,
        after_pid,
        health_observed: true,
    };
    fault_control_occurrence(
        control_type,
        Some(peer_index),
        command_bytes,
        canonical_harness_json_bytes(&acknowledgement)?,
        Some(before_pid),
        Some(after_pid),
    )
}

fn trigger_persistence_cut_and_restart<F>(
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    peer_index: usize,
    phase: NativeAmxFaultPhase,
    bundle_id: Hash,
    restart_revision: u64,
    trigger: F,
) -> Result<Vec<FaultControlOccurrenceV1>>
where
    F: FnOnce() -> Result<()>,
{
    let peer = network.peers()[peer_index].clone();
    let control = peer
        .consensus_message_control()
        .ok_or_else(|| eyre!("crash target lacks persistence control"))?;
    let before_pid = runtime
        .block_on(peer.process_id())
        .ok_or_else(|| eyre!("crash target has no live PID"))?;
    let command = control.arm_native_amx_fault_with_evidence(phase, *bundle_id.as_ref())?;
    let _trigger_result = trigger();
    let acknowledgement = runtime.block_on(control.wait_for_native_amx_fault(
        command.revision,
        phase,
        *bundle_id.as_ref(),
        FAULT_CONTROL_TIMEOUT,
    ))?;
    let (acknowledgement_bytes, readback) = control.read_native_amx_fault_ack_bytes()?;
    ensure!(
        acknowledgement == readback && acknowledgement_bytes == command.canonical_bytes,
        "persistence-cut acknowledgement did not copy the exact fsynced command"
    );
    let persistence = fault_control_occurrence(
        "persistence_cut",
        Some(peer_index),
        command.canonical_bytes,
        acknowledgement_bytes,
        None,
        None,
    )?;
    let config_layers = network.config_layers().collect::<Vec<_>>();
    ensure!(
        runtime.block_on(peer.shutdown_if_started()),
        "crash-cut child was not reapable"
    );
    runtime.block_on(peer.start_checked(config_layers.iter(), None))?;
    let after_pid = runtime
        .block_on(peer.process_id())
        .ok_or_else(|| eyre!("recovered crash target has no PID"))?;
    ensure!(
        before_pid != after_pid && peer.client().get_status().is_ok(),
        "crash recovery did not produce a healthy new process"
    );
    let restart_type = if peer_index < VALIDATORS_PER_LANE {
        "global_restart"
    } else {
        "validator_restart"
    };
    Ok(vec![
        persistence,
        restart_ack_occurrence(
            peer_index,
            restart_revision,
            restart_type,
            before_pid,
            after_pid,
        )?,
    ])
}

fn wait_for_fault_state_reverted(
    network: &Network,
    before: &FaultStateSnapshotV1,
) -> Result<FaultStateSnapshotV1> {
    let started = Instant::now();
    let mut last = None;
    while started.elapsed() <= FINALITY_TIMEOUT {
        match capture_fault_state_snapshot(network, "after") {
            Ok(snapshot) if ensure_fault_state_reverted(before, &snapshot).is_ok() => {
                return Ok(snapshot);
            }
            Ok(_) => last = Some("APS state has not returned to its pre-trial identity".to_owned()),
            Err(error) => last = Some(error.to_string()),
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "timed out waiting for pre-carrier crash reconciliation: {}",
        last.unwrap_or_else(|| "no state response".to_owned())
    ))
}

fn advance_fault_bundle_past_expiry(
    sponsor: &Client,
    bundle: &FaultPreparedBundleV1,
) -> Result<()> {
    loop {
        let height = sponsor.get_privacy_capabilities()?.committed_height;
        if height > bundle.manifest.expiry_height {
            return Ok(());
        }
        let tick = sponsor.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                format!("APS fault expiry {} h{height}", bundle.manifest.bundle_id),
            ))],
            no_fee(),
            Metadata::default(),
        );
        sponsor.submit_transaction_blocking(&tick)?;
    }
}

fn phase_certificate_for_leg(
    sponsor: &Client,
    bundle: &FaultPreparedBundleV1,
    committee: &CommitteeEndpoints,
    leg_ordinal: usize,
    phase: iroha::data_model::nexus::PrivateSettlementPhaseV1,
    barrier: Option<&iroha::data_model::nexus::PrivateSettlementPrepareBarrierV1>,
) -> Result<iroha::data_model::nexus::PrivateSettlementPhaseCertificateV1> {
    let payload = bundle.manifest.legs[leg_ordinal].payload_digest;
    let votes = committee
        .endpoints
        .iter()
        .map(|endpoint| match (phase, barrier) {
            (iroha::data_model::nexus::PrivateSettlementPhaseV1::Prepare, None) => sponsor
                .request_private_settlement_prepare_vote_v1(
                    endpoint,
                    &bundle.manifest,
                    payload,
                    &committee.authority,
                )
                .map(|response| response.vote),
            (iroha::data_model::nexus::PrivateSettlementPhaseV1::Commit, Some(barrier)) => sponsor
                .request_private_settlement_commit_vote_v1(
                    endpoint,
                    payload,
                    barrier,
                    &committee.authority,
                )
                .map(|response| response.vote),
            _ => Err(eyre!("fault phase/barrier combination is invalid")),
        })
        .collect::<Result<Vec<_>>>()?;
    aggregate_fault_phase_votes(
        &votes,
        [0, 1, 2],
        u8::try_from(leg_ordinal).expect("leg ordinal fits u8"),
    )
}

#[derive(Clone, Copy, Debug)]
enum FreshRouteFaultV1 {
    Loss {
        phase: PrivateSettlementRouteControlPhase,
        percentage: u8,
        trial_index: usize,
    },
    Hold {
        phase: PrivateSettlementRouteControlPhase,
        trial_index: usize,
    },
    CarrierHold,
}

#[allow(clippy::too_many_arguments)]
fn run_fresh_route_fault_trial(
    request: &RealProcessFaultRequestV1,
    bundle_ordinal: usize,
    fault: FreshRouteFaultV1,
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    sponsor: &Client,
    routes: &[PrivateSettlementRouteV1],
    committees: &[CommitteeEndpoints],
    coordinator: &mut CoordinatorProcessV1,
) -> Result<(
    FaultTrialDraftV1,
    FaultStateSnapshotV1,
    Option<FaultPrepareQcNormalizationV1>,
    Option<Vec<RealProcessInventoryRowV1>>,
    u64,
)> {
    let mut bundle = prepare_fault_bundle(
        request,
        bundle_ordinal,
        sponsor,
        network,
        routes,
        committees,
    )?;
    let before = wait_for_converged_fault_state_snapshot(network, "before")?;
    let mut controls = Vec::new();
    let mut inventory = None;
    let observer = FaultContinuousObserverV1::start(network, &before, request.participants)?;

    match fault {
        FreshRouteFaultV1::Loss {
            phase: PrivateSettlementRouteControlPhase::RestrictedDa,
            percentage,
            ..
        } => {
            let (observed, nonfinalized) = exercise_route_loss(
                network,
                runtime,
                VALIDATORS_PER_LANE,
                PrivateSettlementRouteControlPhase::RestrictedDa,
                bundle.manifest.bundle_id,
                request.seed,
                percentage,
                || {
                    sponsor
                        .request_private_settlement_availability_share_v1(
                            &committees[0].endpoints[0],
                            &bundle.materials[0],
                        )
                        .map(|_| ())
                },
            )?;
            ensure_fault_ledger_unchanged_before_finality(&before, &nonfinalized)?;
            controls.extend(observed);
        }
        FreshRouteFaultV1::Hold {
            phase: PrivateSettlementRouteControlPhase::RestrictedDa,
            ..
        } => {
            let client = sponsor.clone();
            let endpoint = committees[0].endpoints[0].clone();
            let material = bundle.materials[0].clone();
            let (observed, nonfinalized) = exercise_route_hold(
                network,
                runtime,
                VALIDATORS_PER_LANE,
                PrivateSettlementRouteControlPhase::RestrictedDa,
                bundle.manifest.bundle_id,
                request.seed,
                move || {
                    client
                        .request_private_settlement_availability_share_v1(&endpoint, &material)
                        .map(|_| ())
                },
            )?;
            ensure_fault_ledger_unchanged_before_finality(&before, &nonfinalized)?;
            controls.extend(observed);
        }
        _ => {}
    }
    certify_and_upload_fault_bundle(sponsor, &mut bundle, committees)?;
    audit_fault_bundle(sponsor, &bundle, committees)?;

    match fault {
        FreshRouteFaultV1::Loss {
            phase: PrivateSettlementRouteControlPhase::Prepare,
            percentage,
            ..
        } => {
            let (observed, nonfinalized) = exercise_route_loss(
                network,
                runtime,
                VALIDATORS_PER_LANE,
                PrivateSettlementRouteControlPhase::Prepare,
                bundle.manifest.bundle_id,
                request.seed,
                percentage,
                || {
                    sponsor
                        .request_private_settlement_prepare_vote_v1(
                            &committees[0].endpoints[0],
                            &bundle.manifest,
                            bundle.manifest.legs[0].payload_digest,
                            &committees[0].authority,
                        )
                        .map(|_| ())
                },
            )?;
            ensure_fault_ledger_unchanged_before_finality(&before, &nonfinalized)?;
            controls.extend(observed);
        }
        FreshRouteFaultV1::Hold {
            phase: PrivateSettlementRouteControlPhase::Prepare,
            ..
        } => {
            let client = sponsor.clone();
            let endpoint = committees[0].endpoints[0].clone();
            let manifest = bundle.manifest.clone();
            let authority = committees[0].authority.clone();
            let payload = bundle.manifest.legs[0].payload_digest;
            let (observed, nonfinalized) = exercise_route_hold(
                network,
                runtime,
                VALIDATORS_PER_LANE,
                PrivateSettlementRouteControlPhase::Prepare,
                bundle.manifest.bundle_id,
                request.seed,
                move || {
                    client
                        .request_private_settlement_prepare_vote_v1(
                            &endpoint, &manifest, payload, &authority,
                        )
                        .map(|_| ())
                },
            )?;
            ensure_fault_ledger_unchanged_before_finality(&before, &nonfinalized)?;
            controls.extend(observed);
        }
        _ => {}
    }
    let endpoint_matrix = fault_endpoint_matrix(committees);
    let (barrier, normalization) = if matches!(fault, FreshRouteFaultV1::CarrierHold) {
        let (barrier, normalization) =
            prepare_fault_bundle_with_normalization(sponsor, &bundle, committees)?;
        (barrier, Some(normalization))
    } else {
        (
            sponsor.prepare_private_settlement_bundle_v1(
                &endpoint_matrix,
                &bundle.manifest,
                &bundle.authorities,
                &bundle.deltas,
            )?,
            None,
        )
    };

    match fault {
        FreshRouteFaultV1::Loss {
            phase: PrivateSettlementRouteControlPhase::Commit,
            percentage,
            ..
        } => {
            let (observed, nonfinalized) = exercise_route_loss(
                network,
                runtime,
                VALIDATORS_PER_LANE,
                PrivateSettlementRouteControlPhase::Commit,
                bundle.manifest.bundle_id,
                request.seed,
                percentage,
                || {
                    sponsor
                        .request_private_settlement_commit_vote_v1(
                            &committees[0].endpoints[0],
                            bundle.manifest.legs[0].payload_digest,
                            &barrier,
                            &committees[0].authority,
                        )
                        .map(|_| ())
                },
            )?;
            ensure_fault_ledger_unchanged_before_finality(&before, &nonfinalized)?;
            controls.extend(observed);
        }
        FreshRouteFaultV1::Hold {
            phase: PrivateSettlementRouteControlPhase::Commit,
            ..
        } => {
            let client = sponsor.clone();
            let endpoint = committees[0].endpoints[0].clone();
            let hold_barrier = barrier.clone();
            let authority = committees[0].authority.clone();
            let payload = bundle.manifest.legs[0].payload_digest;
            let (observed, nonfinalized) = exercise_route_hold(
                network,
                runtime,
                VALIDATORS_PER_LANE,
                PrivateSettlementRouteControlPhase::Commit,
                bundle.manifest.bundle_id,
                request.seed,
                move || {
                    client
                        .request_private_settlement_commit_vote_v1(
                            &endpoint,
                            payload,
                            &hold_barrier,
                            &authority,
                        )
                        .map(|_| ())
                },
            )?;
            ensure_fault_ledger_unchanged_before_finality(&before, &nonfinalized)?;
            controls.extend(observed);
        }
        _ => {}
    }
    let unavailable = if matches!(fault, FreshRouteFaultV1::CarrierHold) {
        (0..request.participants)
            .map(|dataspace_ordinal| {
                stop_peer_for_quorum_progress(
                    network,
                    runtime,
                    (dataspace_ordinal + 1) * VALIDATORS_PER_LANE,
                    u64::try_from(dataspace_ordinal + 1).expect("participant ordinal fits u64"),
                )
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    if !unavailable.is_empty() {
        let recovered_while_unavailable = sponsor.recover_or_prepare_private_settlement_bundle_v1(
            &endpoint_matrix,
            &bundle.manifest,
            &bundle.authorities,
            &bundle.deltas,
        )?;
        ensure!(
            recovered_while_unavailable.prepared_bundle_digest == barrier.prepared_bundle_digest,
            "Prepare recovery changed the complete barrier with one validator unavailable per committee"
        );
    }
    let commits = sponsor.commit_private_settlement_bundle_v1(&endpoint_matrix, &barrier)?;
    if !unavailable.is_empty() {
        ensure!(
            commits.len() == request.participants
                && commits.iter().all(|certificate| {
                    certificate.signers_bitmap.count_ones() == 3
                        && certificate.signers_bitmap & 1 == 0
                }),
            "Commit did not make exact 3-of-4 progress with committee seat 0 unavailable"
        );
        for stopped in unavailable {
            controls.push(restart_quorum_progress_peer(network, runtime, stopped)?);
        }
    }
    let (nonfinalized, receipt) = if matches!(fault, FreshRouteFaultV1::CarrierHold) {
        controls.push(restart_peer_with_evidence(
            network,
            runtime,
            0,
            u64::try_from(request.participants + 1)?,
            "global_restart",
        )?);
        let post_restart = capture_fault_state_snapshot(network, "post-restart")?;
        ensure_fault_ledger_unchanged_before_finality(&before, &post_restart)?;
        let coordinator_command = CoordinatorCommandV1 {
            format_version: 1,
            revision: 0,
            operation: "recover_prepare_commit".to_owned(),
            committee_endpoints: endpoint_matrix
                .iter()
                .map(|endpoints| endpoints.iter().map(ToString::to_string).collect())
                .collect(),
            manifest: Some(bundle.manifest.clone()),
            authority_catalog: bundle.authorities.clone(),
            deltas: bundle.deltas.clone(),
            barrier: None,
        };
        let (before_pid, after_pid, command, acknowledgement, recovered) =
            coordinator.restart_with(coordinator_command)?;
        let recovered_barrier = recovered
            .barrier
            .ok_or_else(|| eyre!("coordinator restart did not recover Prepare"))?;
        ensure!(
            recovered_barrier.prepared_bundle_digest == barrier.prepared_bundle_digest
                && recovered.commit_certificates.len() == request.participants,
            "coordinator restart did not recover the exact complete bundle"
        );
        verify_invalid_leg_carrier_is_state_byte_identical(
            sponsor,
            network,
            &bundle,
            &recovered_barrier,
            &recovered.commit_certificates,
        )?;
        controls.push(fault_control_occurrence(
            "coordinator_restart",
            None,
            command,
            acknowledgement,
            Some(before_pid),
            Some(after_pid),
        )?);
        let submit = build_fault_carrier_submit(
            sponsor,
            &bundle,
            &recovered_barrier,
            &recovered.commit_certificates,
        );
        let replay = submit.clone();
        let (carrier_controls, nonfinalized) =
            exercise_consensus_carrier_hold(network, runtime, sponsor, submit)?;
        controls.extend(carrier_controls);
        let receipt = wait_for_identical_receipt(network, bundle.manifest.bundle_id)?;
        ensure!(
            sponsor
                .submit_private_settlement_bundle_v1(&replay)
                .is_err(),
            "fault campaign accepted an exact finalized carrier replay"
        );
        inventory = Some(collect_process_inventory(
            network,
            runtime,
            TopologyShape::new(request.participants),
            &request.commit,
            coordinator,
        )?);
        (nonfinalized, receipt)
    } else {
        let nonfinalized = capture_fault_state_snapshot(network, "nonfinalized")?;
        let receipt = finalize_fault_bundle(sponsor, network, &bundle, barrier, commits)?;
        (nonfinalized, receipt)
    };
    ensure_fault_ledger_unchanged_before_finality(&before, &nonfinalized)?;
    let after = wait_for_converged_fault_state_snapshot(network, "after")?;
    ensure_fault_state_finalized_once(&before, &after, request.participants)?;
    let continuous_observations = observer.finish(&after)?;
    let signed_rs16 = verify_signed_rs16_finality(network, receipt.finalized_height)?;
    let (collection, trial_index) = match fault {
        FreshRouteFaultV1::Loss { trial_index, .. } => ("loss_trials", trial_index),
        FreshRouteFaultV1::Hold { trial_index, .. } => ("phase_cut_partitions", trial_index),
        FreshRouteFaultV1::CarrierHold => ("phase_cut_partitions", 3),
    };
    Ok((
        FaultTrialDraftV1 {
            collection,
            trial_index,
            bundle_id: hex::encode(bundle.manifest.bundle_id.as_ref()),
            expected_after_state: "finalized",
            controls,
            before,
            nonfinalized,
            continuous_observations,
        },
        after,
        normalization,
        inventory,
        signed_rs16,
    ))
}

fn crash_boundary_phase(index: usize) -> Result<NativeAmxFaultPhase> {
    match index {
        0 => Ok(NativeAmxFaultPhase::AfterPrivateSettlementSidecarFsync),
        1 => Ok(NativeAmxFaultPhase::AfterPrivateSettlementStagedDeltaFsync),
        2 => Ok(NativeAmxFaultPhase::AfterPrivateSettlementPrepareQcFsync),
        3 => Ok(NativeAmxFaultPhase::AfterPrivateSettlementCommitQcFsync),
        4 => Ok(NativeAmxFaultPhase::AfterPrivateSettlementKuraAppend),
        5 => Ok(NativeAmxFaultPhase::AfterPrivateSettlementWsvApplication),
        6 => Ok(NativeAmxFaultPhase::AfterPrivateSettlementReceiptPublication),
        _ => Err(eyre!("unknown APS crash boundary")),
    }
}

fn crash_boundary_peer_index(phase: NativeAmxFaultPhase) -> usize {
    if matches!(
        phase,
        NativeAmxFaultPhase::AfterPrivateSettlementKuraAppend
            | NativeAmxFaultPhase::AfterPrivateSettlementWsvApplication
    ) {
        0
    } else {
        VALIDATORS_PER_LANE
    }
}

#[allow(clippy::too_many_arguments)]
fn run_fresh_crash_trial(
    request: &RealProcessFaultRequestV1,
    bundle_ordinal: usize,
    trial_index: usize,
    network: &Network,
    runtime: &tokio::runtime::Runtime,
    sponsor: &Client,
    routes: &[PrivateSettlementRouteV1],
    committees: &[CommitteeEndpoints],
) -> Result<(FaultTrialDraftV1, FaultStateSnapshotV1, u64)> {
    let mut bundle = prepare_fault_bundle(
        request,
        bundle_ordinal,
        sponsor,
        network,
        routes,
        committees,
    )?;
    let before = wait_for_converged_fault_state_snapshot(network, "before")?;
    let observer = FaultContinuousObserverV1::start(network, &before, request.participants)?;
    let phase = crash_boundary_phase(trial_index)?;
    let post_carrier = trial_index >= 4;
    // Kura append and WSV application are global carrier boundaries. Receipt
    // publication is deliberately a participant target: its crash hook follows
    // fsync of that committee's restricted sidecar lifecycle record, which a
    // global-only peer does not possess.
    let peer_index = crash_boundary_peer_index(phase);
    let mut barrier = None;
    let mut commits = None;

    if trial_index > 0 {
        certify_and_upload_fault_bundle(sponsor, &mut bundle, committees)?;
        audit_fault_bundle(sponsor, &bundle, committees)?;
    }
    if trial_index >= 3 {
        let endpoint_matrix = fault_endpoint_matrix(committees);
        barrier = Some(sponsor.prepare_private_settlement_bundle_v1(
            &endpoint_matrix,
            &bundle.manifest,
            &bundle.authorities,
            &bundle.deltas,
        )?);
    }
    if trial_index >= 4 {
        let endpoint_matrix = fault_endpoint_matrix(committees);
        commits = Some(
            sponsor.commit_private_settlement_bundle_v1(
                &endpoint_matrix,
                barrier
                    .as_ref()
                    .ok_or_else(|| eyre!("post-carrier crash lacks Prepare barrier"))?,
            )?,
        );
    }
    let nonfinalized = capture_fault_state_snapshot(network, "nonfinalized")?;
    ensure_fault_ledger_unchanged_before_finality(&before, &nonfinalized)?;

    let controls = match trial_index {
        0 => trigger_persistence_cut_and_restart(
            network,
            runtime,
            peer_index,
            phase,
            bundle.manifest.bundle_id,
            u64::try_from(trial_index + 1)?,
            || {
                sponsor
                    .request_private_settlement_availability_share_v1(
                        &committees[0].endpoints[0],
                        &bundle.materials[0],
                    )
                    .map(|_| ())
            },
        )?,
        1 => trigger_persistence_cut_and_restart(
            network,
            runtime,
            peer_index,
            phase,
            bundle.manifest.bundle_id,
            u64::try_from(trial_index + 1)?,
            || {
                sponsor
                    .request_private_settlement_prepare_vote_v1(
                        &committees[0].endpoints[0],
                        &bundle.manifest,
                        bundle.manifest.legs[0].payload_digest,
                        &committees[0].authority,
                    )
                    .map(|_| ())
            },
        )?,
        2 => {
            let certificate = phase_certificate_for_leg(
                sponsor,
                &bundle,
                &committees[0],
                0,
                iroha::data_model::nexus::PrivateSettlementPhaseV1::Prepare,
                None,
            )?;
            trigger_persistence_cut_and_restart(
                network,
                runtime,
                peer_index,
                phase,
                bundle.manifest.bundle_id,
                u64::try_from(trial_index + 1)?,
                || {
                    sponsor
                        .persist_private_settlement_phase_certificate_v1(
                            &committees[0].endpoints[0],
                            &bundle.manifest,
                            bundle.manifest.legs[0].payload_digest,
                            &certificate,
                        )
                        .map(|_| ())
                },
            )?
        }
        3 => {
            let barrier = barrier
                .as_ref()
                .ok_or_else(|| eyre!("Commit-QC crash lacks Prepare barrier"))?;
            let certificate = phase_certificate_for_leg(
                sponsor,
                &bundle,
                &committees[0],
                0,
                iroha::data_model::nexus::PrivateSettlementPhaseV1::Commit,
                Some(barrier),
            )?;
            trigger_persistence_cut_and_restart(
                network,
                runtime,
                peer_index,
                phase,
                bundle.manifest.bundle_id,
                u64::try_from(trial_index + 1)?,
                || {
                    sponsor
                        .persist_private_settlement_phase_certificate_v1(
                            &committees[0].endpoints[0],
                            &bundle.manifest,
                            bundle.manifest.legs[0].payload_digest,
                            &certificate,
                        )
                        .map(|_| ())
                },
            )?
        }
        4..=6 => {
            let submit = build_fault_carrier_submit(
                sponsor,
                &bundle,
                barrier
                    .as_ref()
                    .ok_or_else(|| eyre!("post-carrier crash lacks Prepare barrier"))?,
                commits
                    .as_ref()
                    .ok_or_else(|| eyre!("post-carrier crash lacks Commit QCs"))?,
            );
            trigger_persistence_cut_and_restart(
                network,
                runtime,
                peer_index,
                phase,
                bundle.manifest.bundle_id,
                u64::try_from(trial_index + 1)?,
                || {
                    sponsor
                        .submit_private_settlement_bundle_v1(&submit)
                        .map(|_| ())
                },
            )?
        }
        _ => unreachable!(),
    };

    let (after, signed_rs16) = if post_carrier {
        let receipt = wait_for_identical_receipt(network, bundle.manifest.bundle_id)?;
        let after = wait_for_converged_fault_state_snapshot(network, "after")?;
        ensure_fault_state_finalized_once(&before, &after, request.participants)?;
        (
            after,
            verify_signed_rs16_finality(network, receipt.finalized_height)?,
        )
    } else {
        advance_fault_bundle_past_expiry(sponsor, &bundle)?;
        let after = wait_for_fault_state_reverted(network, &before)?;
        ensure_fault_state_reverted(&before, &after)?;
        (after, 0)
    };
    let continuous_observations = observer.finish(&after)?;
    Ok((
        FaultTrialDraftV1 {
            collection: "crash_recoveries",
            trial_index,
            bundle_id: hex::encode(bundle.manifest.bundle_id.as_ref()),
            expected_after_state: if post_carrier {
                "finalized"
            } else {
                "reverted"
            },
            controls,
            before,
            nonfinalized,
            continuous_observations,
        },
        after,
        signed_rs16,
    ))
}

fn write_fault_jsonl<T: norito::json::JsonSerialize>(path: &Path, rows: &[T]) -> Result<Vec<u8>> {
    ensure!(!rows.is_empty(), "fault evidence JSONL is empty");
    let mut bytes = Vec::new();
    for row in rows {
        let encoded = canonical_harness_json_bytes(row)?;
        ensure!(
            !encoded.contains(&b'\n') && !encoded.contains(&b'\r'),
            "fault evidence record is not one canonical JSON line"
        );
        bytes.extend_from_slice(&encoded);
        bytes.push(b'\n');
    }
    ensure!(!path.exists(), "fault evidence path already exists");
    write_owner_only_atomic(path, &bytes)?;
    Ok(bytes)
}

fn fault_evidence_root() -> Result<PathBuf> {
    let root = PathBuf::from(
        std::env::var(HARNESS_EVIDENCE_DIR_ENV).wrap_err("missing fault evidence directory")?,
    );
    let canonical = root
        .canonicalize()
        .wrap_err("resolve fault evidence directory")?;
    ensure!(
        canonical == root,
        "fault evidence directory is not canonical"
    );
    let metadata = fs::symlink_metadata(&root)?;
    ensure!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o777 == 0o700
            && fs::read_dir(&root)?.next().is_none(),
        "fault evidence directory is unsafe or non-empty"
    );
    Ok(root)
}

fn materialize_fault_campaign_payload(
    request: &RealProcessFaultRequestV1,
    trials: Vec<(FaultTrialDraftV1, FaultStateSnapshotV1)>,
    normalization: &FaultPrepareQcNormalizationV1,
) -> Result<HarnessJsonValue> {
    ensure!(
        trials.len() == 20,
        "fault campaign trial inventory is incomplete"
    );
    let mut control_rows = Vec::with_capacity(trials.len());
    let mut observation_rows = Vec::with_capacity(trials.len());
    let mut bundle_ids = BTreeSet::new();
    for (draft, after) in &trials {
        ensure!(
            lowercase_digest(&draft.bundle_id, &[64]) && bundle_ids.insert(draft.bundle_id.clone()),
            "fault campaign reused or malformed a fresh bundle id"
        );
        ensure!(
            draft.continuous_observations.len() == (request.participants + 1) * VALIDATORS_PER_LANE
                && draft
                    .continuous_observations
                    .iter()
                    .enumerate()
                    .all(|(peer_index, summary)| summary.peer_index == peer_index),
            "fault continuous observer omitted or reordered validators"
        );
        let continuous_checks =
            draft
                .continuous_observations
                .iter()
                .try_fold(0_u64, |total, summary| {
                    total
                        .checked_add(summary.check_count)
                        .ok_or_else(|| eyre!("fault continuous-check trial total overflow"))
                })?;
        let record = fault_record_id(
            request.participants,
            request.seed,
            request.run,
            draft.collection,
            draft.trial_index,
        );
        control_rows.push(FaultControlRecordV1 {
            record: record.clone(),
            bundle_id: draft.bundle_id.clone(),
            participants: request.participants,
            seed: request.seed,
            run: request.run,
            collection: draft.collection.to_owned(),
            trial_index: draft.trial_index,
            controls: draft.controls.clone(),
        });
        observation_rows.push(FaultObservationRecordV1 {
            record,
            bundle_id: draft.bundle_id.clone(),
            participants: request.participants,
            seed: request.seed,
            run: request.run,
            collection: draft.collection.to_owned(),
            trial_index: draft.trial_index,
            expected_after_state: draft.expected_after_state.to_owned(),
            continuous_checks,
            continuous_observations: draft.continuous_observations.clone(),
            partial_visibility_observed: false,
            partial_spendable_observations: 0,
            snapshots: vec![
                draft.before.clone(),
                draft.nonfinalized.clone(),
                after.clone(),
            ],
        });
    }
    control_rows.sort_by_key(|row| {
        let collection = match row.collection.as_str() {
            "loss_trials" => 0,
            "phase_cut_partitions" => 1,
            "crash_recoveries" => 2,
            _ => 3,
        };
        (collection, row.trial_index)
    });
    observation_rows.sort_by_key(|row| {
        let collection = match row.collection.as_str() {
            "loss_trials" => 0,
            "phase_cut_partitions" => 1,
            "crash_recoveries" => 2,
            _ => 3,
        };
        (collection, row.trial_index)
    });
    let evidence_root = fault_evidence_root()?;
    let control_bytes = write_fault_jsonl(
        &evidence_root.join(FAULT_CONTROL_EVIDENCE_FILE),
        &control_rows,
    )?;
    let observation_bytes = write_fault_jsonl(
        &evidence_root.join(FAULT_OBSERVATION_EVIDENCE_FILE),
        &observation_rows,
    )?;
    let control_sha = sha256_hex(&control_bytes);
    let observation_sha = sha256_hex(&observation_bytes);
    let total_checks = observation_rows.iter().try_fold(0_u64, |total, row| {
        total
            .checked_add(row.continuous_checks)
            .ok_or_else(|| eyre!("fault continuous-check total overflow"))
    })?;

    let make_reference = |collection: &str, trial_index: usize| {
        fault_record_id(
            request.participants,
            request.seed,
            request.run,
            collection,
            trial_index,
        )
    };
    let loss_trials = ["restricted_da", "prepare", "commit"]
        .into_iter()
        .flat_map(|phase| {
            [5_u8, 10, 20]
                .into_iter()
                .map(move |percentage| (phase, percentage))
        })
        .enumerate()
        .map(|(index, (phase, percentage))| {
            norito::json!({
                "phase": phase,
                "loss_percent": percentage,
                "control_acknowledged": true,
                "healed": true,
                "converged": true,
                "partial_visibility_observed": false,
                "control_transcript_sha256": control_sha.clone(),
                "control_transcript_record": make_reference("loss_trials", index),
                "observation_capture_sha256": observation_sha.clone(),
                "observation_capture_record": make_reference("loss_trials", index),
            })
        })
        .collect::<Vec<_>>();
    let phase_cut_partitions = [
        "da_before_availability_qc",
        "prepare_before_complete_barrier",
        "commit_before_complete_barrier",
        "carrier_before_global_finality",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, cut)| {
        norito::json!({
            "cut": cut,
            "control_acknowledged": true,
            "delayed_delivery": true,
            "healed": true,
            "converged": true,
            "partial_visibility_observed": false,
            "control_transcript_sha256": control_sha.clone(),
            "control_transcript_record": make_reference("phase_cut_partitions", index),
            "observation_capture_sha256": observation_sha.clone(),
            "observation_capture_record": make_reference("phase_cut_partitions", index),
        })
    })
    .collect::<Vec<_>>();
    let crash_recoveries = [
        "sidecar_fsync",
        "staged_delta_fsync",
        "prepare_qc",
        "commit_qc",
        "kura_append",
        "wsv_application",
        "receipt_publication",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, boundary)| {
        norito::json!({
            "boundary": boundary,
            "process_restarted": true,
            "durable_state_reconciled": true,
            "converged": true,
            "partial_visibility_observed": false,
            "control_transcript_sha256": control_sha.clone(),
            "control_transcript_record": make_reference("crash_recoveries", index),
            "observation_capture_sha256": observation_sha.clone(),
            "observation_capture_record": make_reference("crash_recoveries", index),
        })
    })
    .collect::<Vec<_>>();
    Ok(norito::json!({
        "committee_validator_restarts": (0..request.participants).collect::<Vec<_>>(),
        "maximum_simultaneously_unavailable_per_committee": 1,
        "quorum_progress_with_one_unavailable": true,
        "coordinator_restarted": true,
        "global_node_restarted": true,
        "prepare_qc_normalization": norito::json::to_value(normalization)?,
        "loss_trials": loss_trials,
        "phase_cut_partitions": phase_cut_partitions,
        "crash_recoveries": crash_recoveries,
        "atomicity": {
            "continuous_checks": total_checks,
            "partial_visible_observations": 0,
            "partial_spendable_observations": 0,
            "aborted_private_state_changes": 0,
            "successful_leg_applications": request.participants,
            "each_leg_applied_exactly_once": true,
            "invalid_leg_state_byte_identical": true,
            "replay_rejected": true,
        },
        "all_nodes_converged": true,
    }))
}

fn run_real_process_fault_campaign(
    request: RealProcessFaultRequestV1,
    request_sha: String,
) -> Result<RealProcessFaultResultV1> {
    let shape = TopologyShape::new(request.participants);
    shape.validate()?;
    let context = format!(
        "atomic_private_settlement_fault_n{}_s{}_r{}",
        request.participants, request.seed, request.run
    );
    let builder = localnet_builder(shape)
        .with_base_seed(format!(
            "atomic-private-settlement-fault-v1-n{}-seed-{}-run-{}",
            request.participants, request.seed, request.run
        ))
        .with_consensus_message_control();
    let started = sandbox::start_network_blocking_or_skip(builder, &context)?;
    let Some((network, runtime)) = sandbox::enforce_network_start_requirement(started, &context)?
    else {
        return Err(eyre!("real-process fault network was skipped"));
    };
    verify_controller_readiness(&network, &runtime)?;
    let sponsor = network.client();
    activate_ivm_private_note(&sponsor)?;
    let routes = routes_from_network(&network, shape)?;
    let committees = committees_from_network(&network, shape, &routes)?;
    ensure!(
        routes.len() == request.participants && committees.len() == request.participants,
        "fault campaign topology material is incomplete"
    );
    let mut coordinator = CoordinatorProcessV1::start(&sponsor)?;
    let mut trials = Vec::with_capacity(20);
    let mut bundle_ordinal = 0_usize;
    let mut signed_rs16_da_observations = 0_u64;

    let route_phases = [
        PrivateSettlementRouteControlPhase::RestrictedDa,
        PrivateSettlementRouteControlPhase::Prepare,
        PrivateSettlementRouteControlPhase::Commit,
    ];
    for (phase_ordinal, phase) in route_phases.into_iter().enumerate() {
        for (percentage_ordinal, percentage) in [5_u8, 10, 20].into_iter().enumerate() {
            let trial_index = phase_ordinal * 3 + percentage_ordinal;
            let (draft, after, observed_normalization, observed_inventory, signed_rs16) =
                run_fresh_route_fault_trial(
                    &request,
                    bundle_ordinal,
                    FreshRouteFaultV1::Loss {
                        phase,
                        percentage,
                        trial_index,
                    },
                    &network,
                    &runtime,
                    &sponsor,
                    &routes,
                    &committees,
                    &mut coordinator,
                )?;
            ensure!(
                observed_normalization.is_none() && observed_inventory.is_none(),
                "ordinary loss trial unexpectedly produced carrier-only evidence"
            );
            signed_rs16_da_observations = signed_rs16_da_observations.max(signed_rs16);
            trials.push((draft, after));
            bundle_ordinal += 1;
        }
    }
    for (trial_index, phase) in route_phases.into_iter().enumerate() {
        let (draft, after, observed_normalization, observed_inventory, signed_rs16) =
            run_fresh_route_fault_trial(
                &request,
                bundle_ordinal,
                FreshRouteFaultV1::Hold { phase, trial_index },
                &network,
                &runtime,
                &sponsor,
                &routes,
                &committees,
                &mut coordinator,
            )?;
        ensure!(
            observed_normalization.is_none() && observed_inventory.is_none(),
            "ordinary phase cut unexpectedly produced carrier-only evidence"
        );
        signed_rs16_da_observations = signed_rs16_da_observations.max(signed_rs16);
        trials.push((draft, after));
        bundle_ordinal += 1;
    }
    let (draft, after, observed_normalization, observed_inventory, signed_rs16) =
        run_fresh_route_fault_trial(
            &request,
            bundle_ordinal,
            FreshRouteFaultV1::CarrierHold,
            &network,
            &runtime,
            &sponsor,
            &routes,
            &committees,
            &mut coordinator,
        )?;
    let normalization = observed_normalization
        .ok_or_else(|| eyre!("carrier trial did not produce QC normalization evidence"))?;
    ensure!(
        observed_inventory.is_some(),
        "carrier trial did not capture a live post-restart inventory"
    );
    signed_rs16_da_observations = signed_rs16_da_observations.max(signed_rs16);
    trials.push((draft, after));
    bundle_ordinal += 1;

    for trial_index in 0..7 {
        let (draft, after, signed_rs16) = run_fresh_crash_trial(
            &request,
            bundle_ordinal,
            trial_index,
            &network,
            &runtime,
            &sponsor,
            &routes,
            &committees,
        )?;
        signed_rs16_da_observations = signed_rs16_da_observations.max(signed_rs16);
        trials.push((draft, after));
        bundle_ordinal += 1;
    }
    ensure!(
        bundle_ordinal == 20
            && signed_rs16_da_observations >= request.minimum_signed_rs16_da_observations,
        "fault campaign did not complete its fresh-bundle/RS16 matrix"
    );
    let process_inventory =
        collect_process_inventory(&network, &runtime, shape, &request.commit, &coordinator)?;
    let payload = materialize_fault_campaign_payload(&request, trials, &normalization)?;
    Ok(RealProcessFaultResultV1 {
        version: 1,
        protocol: "AtomicPrivateSettlementV1".to_owned(),
        request_id: request.request_id,
        invocation_nonce: request.invocation_nonce,
        request_sha256: request_sha,
        commit: request.commit,
        participants: request.participants,
        mandatory_signed_rs16_da_rbc: true,
        signed_rs16_da_observations,
        authenticated_message_control: true,
        process_inventory,
        payload,
    })
}

#[derive(Clone, Copy, Debug)]
struct TransparentControlBalanceExpectation {
    asset_ordinal: usize,
    owner_ordinal: usize,
    amount: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransparentControlSnapshotState {
    Initial,
    Finalized,
}

fn classify_transparent_control_values(
    observed: &[Quantity],
    initial: &[TransparentControlBalanceExpectation],
    finalized: &[TransparentControlBalanceExpectation],
) -> Result<TransparentControlSnapshotState> {
    ensure!(
        observed.len() == initial.len()
            && initial.len() == finalized.len()
            && initial.iter().zip(finalized).all(|(before, after)| {
                before.asset_ordinal == after.asset_ordinal
                    && before.owner_ordinal == after.owner_ordinal
            }),
        "transparent-control atomicity vectors are not aligned"
    );
    let matches = |expected: &[TransparentControlBalanceExpectation]| {
        observed
            .iter()
            .zip(expected)
            .all(|(value, expectation)| *value == Quantity::from(expectation.amount))
    };
    if matches(initial) {
        Ok(TransparentControlSnapshotState::Initial)
    } else if matches(finalized) {
        Ok(TransparentControlSnapshotState::Finalized)
    } else {
        Err(eyre!(
            "transparent-control observer detected a mixed pre/post balance vector"
        ))
    }
}

fn observe_transparent_control_atomicity(
    client: &Client,
    initial: &[TransparentControlBalanceExpectation],
    finalized: &[TransparentControlBalanceExpectation],
) -> Result<TransparentControlSnapshotState> {
    // One FindAssets response is a coherent snapshot from one validator's local WSV. Reading all
    // relevant buckets in that response avoids manufacturing a mixed vector by crossing block
    // boundaries between independent point queries.
    let assets = client.query(FindAssets::new()).execute_all()?;
    let mut observed = Vec::with_capacity(initial.len());
    for expectation in initial {
        let expected_id =
            transparent_control_asset_id(expectation.asset_ordinal, expectation.owner_ordinal);
        let mut matching = assets.iter().filter(|asset| asset.id == expected_id);
        let asset = matching
            .next()
            .ok_or_else(|| eyre!("transparent-control observer cannot find {expected_id}"))?;
        ensure!(
            matching.next().is_none(),
            "transparent-control observer found a duplicate {expected_id}"
        );
        observed.push(asset.value().clone());
    }
    classify_transparent_control_values(&observed, initial, finalized)
}

struct TransparentControlAtomicityObserver {
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    observations: Arc<Vec<AtomicU64>>,
    failure: Arc<Mutex<Option<String>>>,
    handles: Vec<thread::JoinHandle<()>>,
}

impl TransparentControlAtomicityObserver {
    fn start(
        clients: Vec<Client>,
        initial: Vec<TransparentControlBalanceExpectation>,
        finalized: Vec<TransparentControlBalanceExpectation>,
    ) -> Result<Self> {
        ensure!(!clients.is_empty(), "atomicity observer has no clients");
        for client in &clients {
            ensure!(
                observe_transparent_control_atomicity(client, &initial, &finalized)?
                    == TransparentControlSnapshotState::Initial,
                "transparent-control observer did not start from the initial state"
            );
        }
        let observations = Arc::new(
            (0..clients.len())
                .map(|_| AtomicU64::new(1))
                .collect::<Vec<_>>(),
        );
        let stop = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(false));
        let failure = Arc::new(Mutex::new(None));
        let mut client_groups = Vec::<Vec<(usize, Client)>>::new();
        for (index, client) in clients.into_iter().enumerate() {
            let group = index / VALIDATORS_PER_LANE;
            if client_groups.len() == group {
                client_groups.push(Vec::with_capacity(VALIDATORS_PER_LANE));
            }
            client_groups[group].push((index, client));
        }
        let mut handles = Vec::with_capacity(client_groups.len());
        for (group, clients) in client_groups.into_iter().enumerate() {
            let thread_stop = Arc::clone(&stop);
            let thread_active = Arc::clone(&active);
            let thread_observations = Arc::clone(&observations);
            let thread_failure = Arc::clone(&failure);
            let thread_initial = initial.clone();
            let thread_finalized = finalized.clone();
            let handle = match thread::Builder::new()
                .name(format!("aps-transparent-atomicity-observer-{group}"))
                .spawn(move || {
                    while !thread_stop.load(Ordering::Relaxed) {
                        if !thread_active.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        for (index, client) in &clients {
                            if let Err(error) = observe_transparent_control_atomicity(
                                client,
                                &thread_initial,
                                &thread_finalized,
                            ) {
                                if let Ok(mut failure) = thread_failure.lock() {
                                    if failure.is_none() {
                                        *failure = Some(format!("observer#{index}: {error}"));
                                    }
                                }
                                thread_stop.store(true, Ordering::Relaxed);
                                return;
                            }
                            thread_observations[*index].fetch_add(1, Ordering::Relaxed);
                        }
                        thread::sleep(Duration::from_millis(25));
                    }
                }) {
                Ok(handle) => handle,
                Err(error) => {
                    stop.store(true, Ordering::Relaxed);
                    for handle in handles.drain(..) {
                        let _ = handle.join();
                    }
                    return Err(error.into());
                }
            };
            handles.push(handle);
        }
        Ok(Self {
            stop,
            active,
            observations,
            failure,
            handles,
        })
    }

    fn begin(&self) -> Result<()> {
        self.active
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .map_err(|_| eyre!("transparent-control atomicity observer started twice"))?;
        Ok(())
    }

    fn stop_and_join(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        let mut panicked = false;
        for handle in self.handles.drain(..) {
            panicked |= handle.join().is_err();
        }
        ensure!(
            !panicked,
            "one or more transparent-control atomicity observers panicked"
        );
        Ok(())
    }

    fn finish(mut self, minimum_observations_per_validator: u64) -> Result<u64> {
        self.stop_and_join()?;
        if let Some(failure) = self
            .failure
            .lock()
            .map_err(|_| eyre!("transparent-control atomicity observer lock was poisoned"))?
            .take()
        {
            return Err(eyre!(failure));
        }
        let per_validator = self
            .observations
            .iter()
            .map(|observations| observations.load(Ordering::Relaxed))
            .collect::<Vec<_>>();
        ensure!(
            per_validator
                .iter()
                .all(|observations| *observations >= minimum_observations_per_validator),
            "transparent-control atomicity observer missed one or more validators: {per_validator:?}"
        );
        per_validator.iter().try_fold(0_u64, |total, observations| {
            total
                .checked_add(*observations)
                .ok_or_else(|| eyre!("atomicity observation total overflow"))
        })
    }
}

impl Drop for TransparentControlAtomicityObserver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

fn transparent_control_amounts(counterparty_ordinal: usize) -> (u64, u64) {
    let ordinal = u64::try_from(counterparty_ordinal).expect("counterparty ordinal fits u64");
    (10 + ordinal, 20 + ordinal * 2)
}

fn transparent_control_dvps(request: &RealProcessBenchmarkRequestV1) -> Result<Vec<DvpIsi>> {
    let authority = transparent_control_account_id(0);
    (1..request.participants)
        .map(|counterparty_ordinal| {
            let counterparty = transparent_control_account_id(counterparty_ordinal);
            Ok(DvpIsi::new(
                format!(
                    "apsctl_n{}_s{}_r{}_p{}",
                    request.participants, request.seed, request.run, counterparty_ordinal
                )
                .parse()
                .wrap_err("construct transparent-control settlement id")?,
                SettlementLeg::new(
                    transparent_control_asset_definition_id(0),
                    Quantity::from(transparent_control_amounts(counterparty_ordinal).0),
                    authority.clone(),
                    counterparty.clone(),
                ),
                SettlementLeg::new(
                    transparent_control_asset_definition_id(counterparty_ordinal),
                    Quantity::from(transparent_control_amounts(counterparty_ordinal).1),
                    counterparty,
                    authority.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            ))
        })
        .collect()
}

fn transparent_control_balance_expectations(
    participants: usize,
    finalized: bool,
) -> Result<Vec<TransparentControlBalanceExpectation>> {
    let mut expectations = Vec::with_capacity(participants.saturating_mul(3));
    let delivered = (1..participants).try_fold(0_u64, |total, ordinal| {
        total
            .checked_add(transparent_control_amounts(ordinal).0)
            .ok_or_else(|| eyre!("transparent-control delivery total overflow"))
    })?;
    expectations.push(TransparentControlBalanceExpectation {
        asset_ordinal: 0,
        owner_ordinal: 0,
        amount: if finalized {
            TRANSPARENT_CONTROL_SEED_BALANCE
                .checked_sub(delivered)
                .ok_or_else(|| eyre!("transparent-control authority is underfunded"))?
        } else {
            TRANSPARENT_CONTROL_SEED_BALANCE
        },
    });
    for ordinal in 1..participants {
        let (delivery, payment) = transparent_control_amounts(ordinal);
        expectations.push(TransparentControlBalanceExpectation {
            asset_ordinal: 0,
            owner_ordinal: ordinal,
            amount: TRANSPARENT_CONTROL_OUTPUT_BASELINE + if finalized { delivery } else { 0 },
        });
        expectations.push(TransparentControlBalanceExpectation {
            asset_ordinal: ordinal,
            owner_ordinal: 0,
            amount: TRANSPARENT_CONTROL_OUTPUT_BASELINE + if finalized { payment } else { 0 },
        });
        expectations.push(TransparentControlBalanceExpectation {
            asset_ordinal: ordinal,
            owner_ordinal: ordinal,
            amount: TRANSPARENT_CONTROL_SEED_BALANCE - if finalized { payment } else { 0 },
        });
    }
    Ok(expectations)
}

fn wait_for_transparent_control_balances(
    network: &Network,
    shape: TopologyShape,
    expectations: &[TransparentControlBalanceExpectation],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    while started.elapsed() <= FINALITY_TIMEOUT {
        last_observed.clear();
        let mut all_match = true;
        for expectation in expectations {
            let asset_id =
                transparent_control_asset_id(expectation.asset_ordinal, expectation.owner_ordinal);
            let owner_key = transparent_control_keypair(expectation.owner_ordinal);
            let lane = expectation.asset_ordinal + 1;
            for peer_index in shape.validator_range(lane) {
                let client = network.peers()[peer_index]
                    .client_for(asset_id.account(), owner_key.private_key().clone());
                match client.query_single(FindAssetById::new(asset_id.clone())) {
                    Ok(asset) => {
                        let observed = asset.value().clone();
                        let expected = Quantity::from(expectation.amount);
                        if observed != expected {
                            all_match = false;
                        }
                        last_observed.push(format!(
                            "peer#{peer_index}:asset{}:owner{}={observed}",
                            expectation.asset_ordinal, expectation.owner_ordinal
                        ));
                    }
                    Err(error) => {
                        all_match = false;
                        last_observed.push(format!(
                            "peer#{peer_index}:asset{}:owner{}:error={error}",
                            expectation.asset_ordinal, expectation.owner_ordinal
                        ));
                    }
                }
            }
        }
        if all_match {
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: transparent-control balances did not converge: {last_observed:?}"
    ))
}

fn native_receipt_from_diagnostics(
    diagnostics: &SumeragiDiagnosticsStatus,
    source_id: [u8; 32],
) -> Result<Option<NativeAmxReceipt>> {
    diagnostics
        .validate_native_amx_receipts()
        .map_err(|error| eyre!("invalid Native AMX diagnostics receipt: {error}"))?;
    let mut receipts = diagnostics
        .lane_settlement_commitments
        .iter()
        .flat_map(|commitment| &commitment.native_amx_receipts)
        .chain(
            diagnostics
                .lane_relay_envelopes
                .iter()
                .flat_map(|relay| &relay.settlement_commitment.native_amx_receipts),
        )
        .filter(|receipt| receipt.source_id == source_id)
        .cloned()
        .collect::<BTreeSet<_>>();
    ensure!(
        receipts.len() <= 1,
        "diagnostics exposed conflicting Native AMX receipts for one source"
    );
    Ok(receipts.pop_first())
}

fn wait_for_identical_native_amx_receipt(
    network: &Network,
    source_id: [u8; 32],
) -> Result<NativeAmxReceipt> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    while started.elapsed() <= FINALITY_TIMEOUT {
        let mut receipts = Vec::with_capacity(network.peers().len());
        last_observed.clear();
        for (peer_index, peer) in network.peers().iter().enumerate() {
            match peer.client().get_sumeragi_diagnostics() {
                Ok(diagnostics) => match native_receipt_from_diagnostics(&diagnostics, source_id) {
                    Ok(Some(receipt)) => {
                        last_observed.push(format!(
                            "peer#{peer_index}:receipt:legs={}",
                            receipt.legs.len()
                        ));
                        receipts.push(receipt);
                    }
                    Ok(None) => last_observed.push(format!("peer#{peer_index}:pending")),
                    Err(error) => {
                        last_observed.push(format!("peer#{peer_index}:error={error}"));
                    }
                },
                Err(error) => last_observed.push(format!("peer#{peer_index}:error={error}")),
            }
        }
        if receipts.len() == network.peers().len() {
            let expected = receipts[0].clone();
            ensure!(
                receipts.iter().all(|receipt| *receipt == expected),
                "validators exposed different Native AMX receipts for one carrier"
            );
            return Ok(expected);
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "timed out waiting for the production Native AMX receipt on every validator: {last_observed:?}"
    ))
}

fn canonical_carrier_header(
    client: &Client,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> Result<Option<BlockHeader>> {
    let matching = client
        .query(FindBlocks)
        .execute_all()?
        .into_iter()
        .filter(|block| {
            block
                .entrypoint_hashes()
                .any(|observed| observed == entrypoint_hash)
        })
        .map(|block| block.header())
        .collect::<Vec<_>>();
    ensure!(
        matching.len() <= 1,
        "one transaction entrypoint occurs in multiple canonical blocks"
    );
    Ok(matching.into_iter().next())
}

fn wait_for_identical_canonical_carrier(
    network: &Network,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> Result<BlockHeader> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    while started.elapsed() <= FINALITY_TIMEOUT {
        let mut headers = Vec::with_capacity(network.peers().len());
        last_observed.clear();
        for (peer_index, peer) in network.peers().iter().enumerate() {
            match canonical_carrier_header(&peer.client(), entrypoint_hash) {
                Ok(Some(header)) => {
                    last_observed.push(format!(
                        "peer#{peer_index}:h{}:{}",
                        header.height().get(),
                        header.hash()
                    ));
                    headers.push(header);
                }
                Ok(None) => last_observed.push(format!("peer#{peer_index}:pending")),
                Err(error) => last_observed.push(format!("peer#{peer_index}:error={error}")),
            }
        }
        if headers.len() == network.peers().len() {
            let expected = headers[0].clone();
            ensure!(
                headers.iter().all(|header| *header == expected),
                "validators disagree on the canonical Native AMX carrier"
            );
            return Ok(expected);
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "timed out waiting for exact-once canonical carrier convergence: {last_observed:?}"
    ))
}

fn validate_transparent_native_receipt(
    receipt: &NativeAmxReceipt,
    transaction: &SignedTransaction,
    participants: usize,
) -> Result<()> {
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let mut source_id = [0_u8; Hash::LENGTH];
    source_id.copy_from_slice(transaction.hash().as_ref());
    ensure!(
        receipt.source_id == source_id && receipt.legs.len() == participants,
        "Native AMX receipt source or participant count mismatch"
    );
    let expected_routes = (0..participants)
        .map(|ordinal| {
            (
                LaneId::new(u32::try_from(ordinal + 1).expect("lane fits u32")),
                DataSpaceId::new(u64::try_from(ordinal + 1).expect("dataspace fits u64")),
            )
        })
        .collect::<Vec<_>>();
    let observed_routes = receipt
        .legs
        .iter()
        .map(|leg| (leg.lane_id, leg.dataspace_id))
        .collect::<Vec<_>>();
    ensure!(
        observed_routes == expected_routes
            && observed_routes
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                == receipt.legs.len(),
        "Native AMX receipt omitted, duplicated, or reordered a participant route"
    );
    for leg in &receipt.legs {
        let prepare_signers = leg
            .prepare_qc
            .signers_bitmap
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum::<usize>();
        let commit_signers = leg
            .commit_qc
            .signers_bitmap
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum::<usize>();
        ensure!(
            leg.prepare_qc.body.tx_entrypoint_hash == entrypoint_hash
                && leg.commit_qc.body.tx_entrypoint_hash == entrypoint_hash
                && leg.prepare_qc.validator_set().len() == VALIDATORS_PER_LANE
                && leg.commit_qc.validator_set().len() == VALIDATORS_PER_LANE
                && prepare_signers == 3
                && commit_signers == 3,
            "Native AMX receipt lacks an exact 3-of-4 participant certificate"
        );
    }
    Ok(())
}

fn transparent_control_permission(
    settlement: &DvpIsi,
    counterparty_ordinal: usize,
) -> CanExecuteSettlement {
    CanExecuteSettlement {
        debited_asset: transparent_control_asset_id(counterparty_ordinal, counterparty_ordinal),
        settlement_id: settlement.settlement_id().clone(),
        intent_hash: settlement.intent_hash(),
    }
}

fn grant_transparent_control_consents(network: &Network, settlements: &[DvpIsi]) -> Result<()> {
    let authority = transparent_control_account_id(0);
    for (offset, settlement) in settlements.iter().enumerate() {
        let counterparty_ordinal = offset + 1;
        let counterparty_key = transparent_control_keypair(counterparty_ordinal);
        let counterparty = transparent_control_account_id(counterparty_ordinal);
        let client =
            network.peers()[0].client_for(&counterparty, counterparty_key.private_key().clone());
        let permission = transparent_control_permission(settlement, counterparty_ordinal);
        let transaction = client.build_transaction(
            [InstructionBox::from(Grant::account_permission(
                permission,
                authority.clone(),
            ))],
            no_fee(),
            Metadata::default(),
        );
        client
            .submit_transaction_blocking(&transaction)
            .wrap_err_with(|| {
                format!(
                    "commit exact transparent-control consent for counterparty {counterparty_ordinal}"
                )
            })?;
    }
    Ok(())
}

fn wait_for_transparent_control_consents(network: &Network, settlements: &[DvpIsi]) -> Result<()> {
    let started = Instant::now();
    let authority = transparent_control_account_id(0);
    let mut last_observed = Vec::new();
    while started.elapsed() <= FINALITY_TIMEOUT {
        last_observed.clear();
        let mut all_present = true;
        for (offset, settlement) in settlements.iter().enumerate() {
            let counterparty_ordinal = offset + 1;
            let counterparty = transparent_control_account_id(counterparty_ordinal);
            let counterparty_key = transparent_control_keypair(counterparty_ordinal);
            let expected: Permission =
                transparent_control_permission(settlement, counterparty_ordinal).into();
            for (peer_index, peer) in network.peers().iter().enumerate() {
                let client = peer.client_for(&counterparty, counterparty_key.private_key().clone());
                match client
                    .query(FindPermissionsByAccountId::new(counterparty.clone()))
                    .execute_all()
                {
                    Ok(permissions) => {
                        let present = permissions.iter().any(|permission| permission == &expected);
                        all_present &= present;
                        last_observed.push(format!(
                            "peer#{peer_index}:counterparty{counterparty_ordinal}:present={present}"
                        ));
                    }
                    Err(error) => {
                        all_present = false;
                        last_observed.push(format!(
                            "peer#{peer_index}:counterparty{counterparty_ordinal}:error={error}"
                        ));
                    }
                }
            }
        }
        if all_present {
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "transparent-control consents did not converge before measurement: authority={authority}; {last_observed:?}"
    ))
}

fn build_transparent_control_carrier(client: &Client, settlements: &[DvpIsi]) -> SignedTransaction {
    client.build_transaction(
        settlements.iter().cloned().map(InstructionBox::from),
        no_fee(),
        Metadata::default(),
    )
}

fn fresh_transparent_control_replay(
    client: &Client,
    settlements: &[DvpIsi],
    original_entrypoint: HashOf<TransactionEntrypoint>,
) -> Result<SignedTransaction> {
    for _ in 0..100 {
        let candidate = build_transparent_control_carrier(client, settlements);
        if candidate.hash_as_entrypoint() != original_entrypoint {
            return Ok(candidate);
        }
        thread::sleep(Duration::from_millis(1));
    }
    Err(eyre!(
        "could not construct a fresh carrier for settlement-id replay testing"
    ))
}

fn run_real_process_transparent_control_benchmark(
    request: RealProcessBenchmarkRequestV1,
    request_sha: String,
) -> Result<RealProcessBenchmarkResultV1> {
    let shape = TopologyShape::new(request.participants);
    shape.validate()?;
    let context = format!(
        "atomic_private_settlement_transparent_control_n{}_s{}_r{}",
        request.participants, request.seed, request.run
    );
    let builder = localnet_builder(shape)
        .with_base_seed(format!(
            "atomic-private-settlement-transparent-control-v1-n{}-seed-{}-run-{}",
            request.participants, request.seed, request.run
        ))
        .with_consensus_message_control();
    let started = sandbox::start_network_blocking_or_skip(builder, &context)?;
    let Some((network, runtime)) = sandbox::enforce_network_start_requirement(started, &context)?
    else {
        return Err(eyre!("real-process release network was skipped"));
    };
    verify_controller_readiness(&network, &runtime)?;
    let coordinator = CoordinatorProcessV1::start(&network.client())?;
    let inventory =
        collect_process_inventory(&network, &runtime, shape, &request.commit, &coordinator)?;
    let pids = inventory.iter().map(|row| row.pid).collect::<Vec<_>>();
    let authority_key = transparent_control_keypair(0);
    let authority_id = transparent_control_account_id(0);
    let authority =
        network.peers()[0].client_for(&authority_id, authority_key.private_key().clone());
    let settlements = transparent_control_dvps(&request)?;
    ensure!(
        settlements.len() == request.participants - 1,
        "transparent-control DvP batch does not cover every counterparty"
    );
    grant_transparent_control_consents(&network, &settlements)?;
    wait_for_transparent_control_consents(&network, &settlements)?;
    let initial_balances = transparent_control_balance_expectations(request.participants, false)?;
    wait_for_transparent_control_balances(
        &network,
        shape,
        &initial_balances,
        "transparent-control pre-state",
    )?;
    let final_balances = transparent_control_balance_expectations(request.participants, true)?;
    let atomicity_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    ensure!(
        atomicity_clients.len() == shape.peer_count(),
        "transparent-control atomicity observer does not cover every validator"
    );
    let atomicity_observer = TransparentControlAtomicityObserver::start(
        atomicity_clients.clone(),
        initial_balances.clone(),
        final_balances.clone(),
    )?;

    let process_before = sample_process_resources(&pids)?;
    let sampler = ProcessResourceSampler::start(pids.clone(), process_before.rss_bytes)?;
    let network_before = loopback_bytes()?;
    let storage_before = network_storage_bytes(&network)?;
    let end_to_end_started = Instant::now();
    atomicity_observer.begin()?;
    let transaction = build_transparent_control_carrier(&authority, &settlements);
    let transaction_hash = transaction.hash();
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let mut source_id = [0_u8; Hash::LENGTH];
    source_id.copy_from_slice(transaction_hash.as_ref());
    let finality_started = Instant::now();
    authority
        .submit_transaction_blocking(&transaction)
        .wrap_err("submit production transparent Native AMX carrier")?;
    let receipt = wait_for_identical_native_amx_receipt(&network, source_id)?;
    validate_transparent_native_receipt(&receipt, &transaction, request.participants)?;
    let carrier = wait_for_identical_canonical_carrier(&network, entrypoint_hash)?;
    ensure!(
        receipt.authority_context_height == carrier.height().get(),
        "Native AMX receipt authority context differs from its canonical carrier"
    );
    let global_finality = elapsed_ms(finality_started);
    wait_for_transparent_control_balances(
        &network,
        shape,
        &final_balances,
        "transparent-control finalized state",
    )?;
    for client in &atomicity_clients {
        ensure!(
            observe_transparent_control_atomicity(client, &initial_balances, &final_balances)?
                == TransparentControlSnapshotState::Finalized,
            "transparent-control atomicity observer did not reach the finalized state"
        );
    }
    let _atomicity_observations = atomicity_observer.finish(3)?;
    let end_to_end = elapsed_ms(end_to_end_started);

    // Transparent DvP/PvP V1 is bilateral. The N-participant control is therefore one
    // production carrier containing N-1 bilateral star DvPs. Native AMX deduplicates their
    // routes into exactly N certified participant legs, and the enclosing StateTransaction
    // commits or rejects the complete DvP batch atomically.
    let replay = fresh_transparent_control_replay(&authority, &settlements, entrypoint_hash)?;
    let replay_entrypoint = replay.hash_as_entrypoint();
    authority
        .submit_transaction_blocking(&replay)
        .expect_err("a fresh carrier reusing committed settlement ids was accepted");
    wait_for_transparent_control_balances(
        &network,
        shape,
        &final_balances,
        "transparent-control state after rejected replay",
    )?;
    ensure!(
        wait_for_identical_native_amx_receipt(&network, source_id)? == receipt
            && wait_for_identical_canonical_carrier(&network, entrypoint_hash)? == carrier,
        "replay changed the durable Native AMX receipt or canonical carrier"
    );
    for peer in network.peers() {
        ensure!(
            canonical_carrier_header(&peer.client(), replay_entrypoint)?.is_none(),
            "rejected settlement-id replay appeared in canonical history"
        );
    }

    let signed_rs16_da_observations =
        verify_signed_rs16_finality(&network, carrier.height().get())?;
    ensure!(
        signed_rs16_da_observations >= request.minimum_signed_rs16_da_observations,
        "signed RS16 finality observations are incomplete"
    );
    let receipt_bytes =
        u64::try_from(norito::encode_canonical(&receipt)?.len()).expect("receipt length fits u64");
    let storage_after = network_storage_bytes(&network)?;
    let network_after = loopback_bytes()?;
    let process_after = sample_process_resources(&pids)?;
    let peak_rss_bytes = sampler.finish(process_after.rss_bytes)?;
    let cpu_seconds = process_after.cpu_seconds - process_before.cpu_seconds;
    let network_bytes = network_after
        .checked_sub(network_before)
        .ok_or_else(|| eyre!("loopback counter moved backwards"))?;
    let storage_growth_bytes = storage_after
        .checked_sub(storage_before)
        .ok_or_else(|| eyre!("Kura storage shrank during benchmark"))?;
    ensure!(
        global_finality > 0.0
            && end_to_end > 0.0
            && cpu_seconds > 0.0
            && peak_rss_bytes > 0
            && network_bytes > 0
            && receipt_bytes > 0,
        "one or more required genuine transparent-control measurements is empty"
    );
    Ok(RealProcessBenchmarkResultV1 {
        version: 1,
        protocol: "AtomicPrivateSettlementV1".to_owned(),
        request_id: request.request_id,
        invocation_nonce: request.invocation_nonce,
        request_sha256: request_sha,
        commit: request.commit,
        participants: request.participants,
        mandatory_signed_rs16_da_rbc: true,
        signed_rs16_da_observations,
        authenticated_message_control: true,
        process_inventory: inventory,
        payload: RealProcessBenchmarkResultPayloadV1 {
            stages_ms: norito::json!({
                "global_finality": global_finality,
                "end_to_end": end_to_end,
            }),
            throughput_bundles_per_second: 1_000.0 / end_to_end,
            cpu_seconds,
            peak_rss_bytes,
            network_bytes,
            // Transparent Native AMX carries no private proof bytes by construction.
            proof_bytes: 0,
            receipt_bytes,
            storage_growth_bytes,
            finalized_receipt_observed: true,
            successful_leg_applications: receipt.legs.len(),
            each_leg_applied_exactly_once: true,
            // The full-topology observer rejected every mixed balance snapshot.
            partial_visible_observations: 0,
            partial_spendable_observations: 0,
        },
    })
}

fn verify_committee_proof_views(
    sponsor: &Client,
    manifest: &AtomicPrivateSettlementV1,
    prepared: &[PreparedLeg],
    committees: &[CommitteeEndpoints],
) -> Result<()> {
    for (ordinal, (leg, committee)) in prepared.iter().zip(committees).enumerate() {
        ensure!(
            committee.endpoints.len() == VALIDATORS_PER_LANE
                && committee.validator_keys.len() == VALIDATORS_PER_LANE,
            "committee proof matrix is incomplete"
        );
        for (endpoint, key) in committee.endpoints.iter().zip(&committee.validator_keys) {
            let mut client = sponsor.clone();
            client.torii_url = endpoint.clone();
            let view = client.private_settlement_committee_proof_v1(
                manifest.legs[ordinal].payload_digest,
                key,
            )?;
            ensure!(
                view.manifest == *manifest
                    && view.committee_authority == committee.authority
                    && view.statement == leg.statement
                    && view.proof == leg.proof
                    && view.delta == leg.delta
                    && view.audit_capsule_digest == leg.capsule.digest()?
                    && !view.audit_approvals.is_empty()
                    && view.lifecycle == PrivateSettlementLifecycleDtoV1::Audited,
                "committee proof view was substituted or incomplete"
            );
        }
    }
    Ok(())
}

fn run_real_process_private_benchmark(
    request: RealProcessBenchmarkRequestV1,
    request_sha: String,
) -> Result<RealProcessBenchmarkResultV1> {
    let shape = TopologyShape::new(request.participants);
    shape.validate()?;
    let context = format!(
        "atomic_private_settlement_real_process_n{}_s{}_r{}",
        request.participants, request.seed, request.run
    );
    let builder = localnet_builder(shape)
        .with_base_seed(format!(
            "atomic-private-settlement-real-process-v1-n{}-seed-{}-run-{}",
            request.participants, request.seed, request.run
        ))
        .with_consensus_message_control();
    let started = sandbox::start_network_blocking_or_skip(builder, &context)?;
    let Some((network, runtime)) = sandbox::enforce_network_start_requirement(started, &context)?
    else {
        return Err(eyre!("real-process release network was skipped"));
    };
    verify_controller_readiness(&network, &runtime)?;
    let coordinator = CoordinatorProcessV1::start(&network.client())?;
    let inventory =
        collect_process_inventory(&network, &runtime, shape, &request.commit, &coordinator)?;
    let pids = inventory.iter().map(|row| row.pid).collect::<Vec<_>>();
    let sponsor = network.client();
    let activated_height = activate_ivm_private_note(&sponsor)?;
    let authority_context_height = activated_height + 1;
    let expiry_height = authority_context_height + 1_000;
    let routes = routes_from_network(&network, shape)?;
    ensure!(routes.len() == request.participants, "route count mismatch");
    let committees = committees_from_network(&network, shape, &routes)?;
    let governed = governed_legs(&routes, authority_context_height, expiry_height)?;
    let manifest = proof_manifest(
        network.network_id(),
        authority_context_height,
        expiry_height,
        &governed,
    )?;

    let process_before = sample_process_resources(&pids)?;
    let sampler = ProcessResourceSampler::start(pids.clone(), process_before.rss_bytes)?;
    let network_before = loopback_bytes()?;
    let storage_before = network_storage_bytes(&network)?;
    let end_to_end_started = Instant::now();

    let proof_started = Instant::now();
    let prepared = governed
        .into_iter()
        .zip(&committees)
        .enumerate()
        .map(|(ordinal, (leg, committee))| {
            prepare_leg(ordinal, leg, &manifest, committee.authority.digest()?)
        })
        .collect::<Result<Vec<_>>>()?;
    let proof_generation = elapsed_ms(proof_started);
    let proof_bytes = prepared.iter().try_fold(0_u64, |total, leg| {
        total
            .checked_add(u64::try_from(leg.proof.len()).expect("proof length fits u64"))
            .ok_or_else(|| eyre!("proof byte total overflow"))
    })?;

    let activations = prepared
        .iter()
        .map(|leg| {
            ActivatePrivateSettlementPoolV1::from_restricted(
                &leg.governed.governance,
                leg.initial_commitments.to_vec(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let activation_transaction =
        sponsor.build_transaction_from_items(activations, no_fee(), Metadata::default());
    sponsor
        .submit_transaction_blocking(&activation_transaction)
        .wrap_err("activate governed private pools")?;
    ensure!(
        sponsor.get_privacy_capabilities()?.committed_height == authority_context_height,
        "pool activation did not land at the manifest authority context"
    );

    let upload_started = Instant::now();
    let materials = provisional_materials(manifest, &prepared, &committees)?;
    let certificates = materials
        .iter()
        .zip(&committees)
        .map(|(material, committee)| {
            sponsor.certify_private_settlement_leg_availability_v1(&committee.endpoints, material)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut final_manifest = materials[0].manifest.clone();
    for (ordinal, certificate) in certificates.iter().enumerate() {
        final_manifest.legs[ordinal].availability_certificate_digest = certificate.digest()?;
    }
    final_manifest.validate()?;
    for (ordinal, ((material, certificate), committee)) in materials
        .iter()
        .zip(&certificates)
        .zip(&committees)
        .enumerate()
    {
        let upload = PrivateSettlementLegUploadRequestV1 {
            manifest: final_manifest.clone(),
            audit_policy: material.audit_policy.clone(),
            committee_authority: material.committee_authority.clone(),
            payload: material.payload_with_certificate(certificate.clone()),
        };
        for endpoint in &committee.endpoints {
            let response = sponsor.upload_private_settlement_leg_to_v1(endpoint, &upload)?;
            ensure!(
                usize::from(response.leg_ordinal) == ordinal,
                "upload ordinal substitution"
            );
        }
    }
    let restricted_upload_availability = elapsed_ms(upload_started);
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "collecting")?;

    let auditor_started = Instant::now();
    for (ordinal, (leg, committee)) in prepared.iter().zip(&committees).enumerate() {
        let fetched = sponsor.private_settlement_auditor_capsule_v1(
            final_manifest.legs[ordinal].payload_digest,
            &leg.governed.auditor_signing,
        )?;
        ensure!(
            fetched.lifecycle == PrivateSettlementLifecycleDtoV1::Collecting,
            "unexpected audit lifecycle"
        );
        let view = PrivateSettlementAuditorSidecarViewV1 {
            manifest: fetched.manifest,
            policy: fetched.audit_policy,
            authority: fetched.committee_authority,
            statement: fetched.statement,
            delta: fetched.delta,
            audit_capsule: fetched.audit_capsule,
            availability: fetched.availability,
            lifecycle: PrivateSettlementSidecarLifecycleV1::Collecting,
        };
        let auditor_id = AccountId::new(leg.governed.auditor_signing.public_key().clone());
        let approval = approve_private_settlement_leg_v1(
            &view,
            &leg.governed.governance,
            authority_context_height,
            &auditor_id,
            leg.governed.auditor_encryption.secret(),
            &leg.governed.auditor_signing,
            &approve_all_audit_material,
        )?;
        for endpoint in &committee.endpoints {
            let mut endpoint_client = sponsor.clone();
            endpoint_client.torii_url = endpoint.clone();
            let response = endpoint_client.submit_private_settlement_audit_approval_v1(
                final_manifest.legs[ordinal].payload_digest,
                &leg.governed.auditor_signing,
                &PrivateSettlementAuditApprovalRequestV1 {
                    approval: approval.clone(),
                },
            )?;
            ensure!(
                response.lifecycle == PrivateSettlementLifecycleDtoV1::Audited,
                "approval was not durable"
            );
        }
    }
    let auditor_response = elapsed_ms(auditor_started);
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "audited")?;

    let committee_started = Instant::now();
    verify_committee_proof_views(&sponsor, &final_manifest, &prepared, &committees)?;
    let committee_verification = elapsed_ms(committee_started);

    let endpoint_matrix = committees
        .iter()
        .map(|committee| committee.endpoints.clone())
        .collect::<Vec<_>>();
    let authorities = committees
        .iter()
        .map(|committee| committee.authority.clone())
        .collect::<Vec<_>>();
    let deltas = prepared
        .iter()
        .map(|leg| leg.delta.clone())
        .collect::<Vec<_>>();
    let prepare_started = Instant::now();
    let barrier = sponsor.prepare_private_settlement_bundle_v1(
        &endpoint_matrix,
        &final_manifest,
        &authorities,
        &deltas,
        &deltas,
    )?;
    let prepare = elapsed_ms(prepare_started);
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "prepared")?;
    let commit_started = Instant::now();
    let commits = sponsor.commit_private_settlement_bundle_v1(&endpoint_matrix, &barrier)?;
    let commit = elapsed_ms(commit_started);
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "commit-certified")?;

    let legs = deltas
        .into_iter()
        .zip(barrier.prepare_certificates)
        .zip(commits)
        .map(|((delta, prepare), commit)| PrivateSettlementLegReceiptV1 {
            delta,
            prepare,
            commit,
        })
        .collect();
    let carrier = FinalizeAtomicPrivateSettlementV1::new(PrivateSettlementCommitBundleV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest: final_manifest.clone(),
        authority_catalog: authorities,
        legs,
    });
    let transaction = sponsor.build_transaction(
        [InstructionBox::from(carrier)],
        final_manifest.public_fee_intent.clone(),
        Metadata::default(),
    );
    let submit = PrivateSettlementBundleSubmitRequestV1 {
        transaction: transaction.clone(),
    };
    let finality_started = Instant::now();
    sponsor.submit_private_settlement_bundle_v1(&submit)?;
    let receipt = wait_for_identical_receipt(&network, final_manifest.bundle_id)?;
    let global_finality = elapsed_ms(finality_started);
    let end_to_end = elapsed_ms(end_to_end_started);
    ensure!(
        receipt.legs.len() == request.participants,
        "receipt participant count mismatch"
    );
    for (ordinal, leg) in receipt.legs.iter().enumerate() {
        ensure!(
            usize::from(leg.delta.leg_ordinal) == ordinal
                && leg.delta == prepared[ordinal].delta
                && receipt
                    .legs
                    .iter()
                    .filter(|candidate| candidate.delta.route == leg.delta.route)
                    .count()
                    == 1,
            "receipt omitted, duplicated, or reordered a private leg"
        );
    }
    ensure!(
        sponsor
            .submit_private_settlement_bundle_v1(&submit)
            .is_err(),
        "replaying the exact finalized carrier was accepted"
    );
    ensure!(
        wait_for_identical_receipt(&network, final_manifest.bundle_id)? == receipt,
        "replay changed the terminal receipt"
    );
    let signed_rs16_da_observations =
        verify_signed_rs16_finality(&network, receipt.finalized_height)?;
    ensure!(
        signed_rs16_da_observations >= request.minimum_signed_rs16_da_observations,
        "signed RS16 finality observations are incomplete"
    );
    let receipt_bytes =
        u64::try_from(norito::encode_canonical(&receipt)?.len()).expect("receipt length fits u64");
    let storage_after = network_storage_bytes(&network)?;
    let network_after = loopback_bytes()?;
    let process_after = sample_process_resources(&pids)?;
    let peak_rss_bytes = sampler.finish(process_after.rss_bytes)?;
    let cpu_seconds = process_after.cpu_seconds - process_before.cpu_seconds;
    let network_bytes = network_after
        .checked_sub(network_before)
        .ok_or_else(|| eyre!("loopback counter moved backwards"))?;
    let storage_growth_bytes = storage_after
        .checked_sub(storage_before)
        .ok_or_else(|| eyre!("Kura storage shrank during benchmark"))?;
    ensure!(
        proof_generation > 0.0
            && restricted_upload_availability > 0.0
            && auditor_response > 0.0
            && committee_verification > 0.0
            && prepare > 0.0
            && commit > 0.0
            && global_finality > 0.0
            && end_to_end > 0.0
            && cpu_seconds > 0.0
            && peak_rss_bytes > 0
            && network_bytes > 0
            && proof_bytes > 0
            && receipt_bytes > 0,
        "one or more required genuine benchmark measurements is empty"
    );
    Ok(RealProcessBenchmarkResultV1 {
        version: 1,
        protocol: "AtomicPrivateSettlementV1".to_owned(),
        request_id: request.request_id,
        invocation_nonce: request.invocation_nonce,
        request_sha256: request_sha,
        commit: request.commit,
        participants: request.participants,
        mandatory_signed_rs16_da_rbc: true,
        signed_rs16_da_observations,
        authenticated_message_control: true,
        process_inventory: inventory,
        payload: RealProcessBenchmarkResultPayloadV1 {
            stages_ms: norito::json!({
                "proof_generation": proof_generation,
                "restricted_upload_availability": restricted_upload_availability,
                "auditor_response": auditor_response,
                "committee_verification": committee_verification,
                "prepare": prepare,
                "commit": commit,
                "global_finality": global_finality,
                "end_to_end": end_to_end,
            }),
            throughput_bundles_per_second: 1_000.0 / end_to_end,
            cpu_seconds,
            peak_rss_bytes,
            network_bytes,
            proof_bytes,
            receipt_bytes,
            storage_growth_bytes,
            finalized_receipt_observed: true,
            successful_leg_applications: receipt.legs.len(),
            each_leg_applied_exactly_once: true,
            // These benchmark zeroes are backed only by the collecting/audited/prepared/
            // commit-certified phase-boundary assertions above. The separate fault campaign
            // emits full-topology continuous-observer evidence.
            // TODO: Add that observer here before describing benchmark rows themselves as
            // continuously observed atomicity evidence.
            partial_visible_observations: 0,
            partial_spendable_observations: 0,
        },
    })
}

fn write_real_process_result<T: norito::json::JsonSerialize>(result: &T) -> Result<()> {
    let path = PathBuf::from(
        std::env::var(HARNESS_RESULT_ENV).wrap_err("missing real-process result path")?,
    );
    ensure!(!path.exists(), "real-process result path already exists");
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("real-process result has no parent"))?;
    let parent_metadata = fs::symlink_metadata(parent).wrap_err("inspect result parent")?;
    ensure!(
        parent_metadata.file_type().is_dir(),
        "real-process result parent is not a regular directory"
    );
    let encoded = format!("{}\n", norito::json::to_string_pretty(result)?);
    let temporary = parent.join(format!(".aps-rust-result-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .wrap_err("create temporary Rust result")?;
    file.write_all(encoded.as_bytes())?;
    file.flush()?;
    file.sync_all()?;
    fs::hard_link(&temporary, &path).wrap_err("atomically publish Rust result")?;
    fs::remove_file(&temporary)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[test]
#[ignore = "release-only: starts 12-68 real validators and runs private or transparent Native AMX"]
fn atomic_private_settlement_real_process_benchmark_harness() -> Result<()> {
    let handle = thread::Builder::new()
        .name("atomic-private-settlement-real-process-harness".to_owned())
        .stack_size(TEST_STACK_BYTES)
        .spawn(|| {
            let (bound, request_sha) = read_bound_real_process_request()?;
            let RealProcessBoundRequestV1::Benchmark(request) = bound else {
                return Err(eyre!(
                    "benchmark entrypoint received a non-benchmark request"
                ));
            };
            let profile = request.payload.profile.clone();
            let result = match profile.as_str() {
                "private" => run_real_process_private_benchmark(request, request_sha)?,
                "transparent_control" => {
                    run_real_process_transparent_control_benchmark(request, request_sha)?
                }
                _ => return Err(eyre!("unsupported real-process benchmark profile")),
            };
            write_real_process_result(&result)
        })
        .expect("spawn real-process release harness thread");
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[test]
#[ignore = "release-only: starts 12-68 validators and runs the complete authenticated fault matrix"]
fn atomic_private_settlement_real_process_fault_harness() -> Result<()> {
    let handle = thread::Builder::new()
        .name("atomic-private-settlement-real-process-fault-harness".to_owned())
        .stack_size(TEST_STACK_BYTES)
        .spawn(|| {
            let (bound, request_sha) = read_bound_real_process_request()?;
            let RealProcessBoundRequestV1::Fault(request) = bound else {
                return Err(eyre!("fault entrypoint received a non-fault request"));
            };
            let result = run_real_process_fault_campaign(request, request_sha)?;
            write_real_process_result(&result)
        })
        .expect("spawn real-process fault release harness thread");
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[test]
#[ignore = "release-only child: owner-only restartable private-settlement coordinator"]
fn atomic_private_settlement_real_process_coordinator_helper() -> Result<()> {
    run_coordinator_helper_process()
}

#[test]
fn real_process_request_digest_validation_is_fail_closed() {
    assert!(lowercase_digest(&"a".repeat(64), &[64]));
    assert!(lowercase_digest(&"b".repeat(40), &[40, 64]));
    assert!(!lowercase_digest(&"0".repeat(64), &[64]));
    assert!(!lowercase_digest(&"A".repeat(64), &[64]));
    assert!(!lowercase_digest("not-a-digest", &[64]));
}

fn fault_observation_fixture(
    peer_index: usize,
    ledger_byte: char,
    roots: u64,
) -> FaultStateObservationV1 {
    let finalized = roots != 0;
    FaultStateObservationV1 {
        peer_index,
        response_sha256: if finalized { "2" } else { "1" }.repeat(64),
        response_hex: "7b7d".to_owned(),
        height: 1,
        commitment: ledger_byte.to_string().repeat(64),
        ledger_commitment: ledger_byte.to_string().repeat(64),
        staged_lock_commitment: "3".repeat(64),
        counts: norito::json!({
            "governance": 1,
            "pools": 1,
            "roots": roots,
            "nullifiers": if finalized { 4 } else { 0 },
            "commitments": if finalized { 6 } else { 0 },
            "encrypted_outputs": if finalized { 6 } else { 0 },
            "replay_markers": if finalized { 1 } else { 0 },
            "receipts": if finalized { 1 } else { 0 },
            "abort_markers": 0,
            "staged_pool_heads": 0,
            "staged_nullifiers": 0,
            "staged_output_commitments": 0,
            "staged_locks": 0,
        }),
    }
}

#[test]
fn fault_continuous_observer_accepts_only_baseline_or_complete_finalization() {
    let baseline = fault_observation_fixture(0, 'a', 0);
    let finalized = fault_observation_fixture(0, 'b', 2);
    assert_eq!(
        classify_fault_continuous_observation(&baseline, &baseline, 2).unwrap(),
        FaultContinuousObservationClassV1::Baseline
    );
    assert_eq!(
        classify_fault_continuous_observation(&baseline, &finalized, 2).unwrap(),
        FaultContinuousObservationClassV1::Finalized
    );
    let partial = fault_observation_fixture(0, 'c', 1);
    assert!(classify_fault_continuous_observation(&baseline, &partial, 2).is_err());

    let mut accumulator = FaultContinuousObservationAccumulatorV1::new(0);
    accumulator.record(&baseline, &baseline, 2).unwrap();
    accumulator.record(&baseline, &baseline, 2).unwrap();
    accumulator.record(&baseline, &finalized, 2).unwrap();
    let summary = accumulator.finish().unwrap();
    assert_eq!(summary.check_count, 3);
    assert_eq!(summary.baseline_observations, 2);
    assert_eq!(summary.finalized_observations, 1);
    assert_eq!(summary.first_response_sha256, baseline.response_sha256);
    assert_eq!(summary.last_response_sha256, finalized.response_sha256);
}

#[test]
fn fault_crash_boundary_inventory_is_exact() {
    let expected = [
        NativeAmxFaultPhase::AfterPrivateSettlementSidecarFsync,
        NativeAmxFaultPhase::AfterPrivateSettlementStagedDeltaFsync,
        NativeAmxFaultPhase::AfterPrivateSettlementPrepareQcFsync,
        NativeAmxFaultPhase::AfterPrivateSettlementCommitQcFsync,
        NativeAmxFaultPhase::AfterPrivateSettlementKuraAppend,
        NativeAmxFaultPhase::AfterPrivateSettlementWsvApplication,
        NativeAmxFaultPhase::AfterPrivateSettlementReceiptPublication,
    ];
    for (index, expected) in expected.into_iter().enumerate() {
        let phase = crash_boundary_phase(index).unwrap();
        assert_eq!(phase, expected);
        assert_eq!(
            crash_boundary_peer_index(phase),
            if matches!(index, 4 | 5) {
                0
            } else {
                VALIDATORS_PER_LANE
            }
        );
    }
    assert!(crash_boundary_phase(7).is_err());
}

#[test]
fn ps_cpu_time_parser_accepts_portable_shapes_and_rejects_malformed_values() {
    assert_eq!(parse_ps_cpu_time("01:02").unwrap(), 62.0);
    assert_eq!(parse_ps_cpu_time("01:02:03.5").unwrap(), 3_723.5);
    assert_eq!(parse_ps_cpu_time("2-01:02:03").unwrap(), 176_523.0);
    assert!(parse_ps_cpu_time("broken").is_err());
}

#[test]
fn real_process_benchmark_stage_inventory_is_profile_exact() {
    assert_eq!(
        benchmark_stages("private").unwrap(),
        PRIVATE_BENCHMARK_STAGES
    );
    assert_eq!(
        benchmark_stages("transparent_control").unwrap(),
        TRANSPARENT_CONTROL_BENCHMARK_STAGES
    );
    assert!(benchmark_stages("ordinary-transfer-substitute").is_err());
}

#[test]
fn transparent_control_identifiers_are_distinct_and_dataspace_scoped() {
    let shape = TopologyShape::new(16);
    shape.validate().unwrap();
    let accounts = (0..shape.participants)
        .map(transparent_control_account_id)
        .collect::<BTreeSet<_>>();
    let definitions = (0..shape.participants)
        .map(transparent_control_asset_definition_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(accounts.len(), shape.participants);
    assert_eq!(definitions.len(), shape.participants);
    for ordinal in 0..shape.participants {
        assert_eq!(
            transparent_control_asset_id(ordinal, ordinal).scope(),
            &AssetBalanceScope::Dataspace(DataSpaceId::new(u64::try_from(ordinal + 1).unwrap()))
        );
    }
}

#[test]
fn transparent_control_atomicity_classifier_rejects_mixed_vectors() {
    let initial = transparent_control_balance_expectations(3, false).unwrap();
    let finalized = transparent_control_balance_expectations(3, true).unwrap();
    let initial_values = initial
        .iter()
        .map(|expectation| Quantity::from(expectation.amount))
        .collect::<Vec<_>>();
    let finalized_values = finalized
        .iter()
        .map(|expectation| Quantity::from(expectation.amount))
        .collect::<Vec<_>>();
    assert_eq!(
        classify_transparent_control_values(&initial_values, &initial, &finalized).unwrap(),
        TransparentControlSnapshotState::Initial
    );
    assert_eq!(
        classify_transparent_control_values(&finalized_values, &initial, &finalized).unwrap(),
        TransparentControlSnapshotState::Finalized
    );
    let mut mixed = initial_values;
    mixed[1] = finalized_values[1].clone();
    assert!(classify_transparent_control_values(&mixed, &initial, &finalized).is_err());
}
