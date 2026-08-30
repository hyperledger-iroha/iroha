use iroha::data_model::block::consensus_v2::SumeragiV2GenesisContextParameters;
use norito::json::Value as HarnessJsonValue;
use sha2::{Digest as Sha2Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const HARNESS_REQUEST_ENV: &str = "APS_REAL_PROCESS_REQUEST";
const HARNESS_RESULT_ENV: &str = "APS_REAL_PROCESS_RESULT";
const HARNESS_REQUEST_SHA_ENV: &str = "APS_REAL_PROCESS_REQUEST_SHA256";
const HARNESS_VALIDATOR_SHA_ENV: &str = "APS_REAL_PROCESS_VALIDATOR_SHA256";
const HARNESS_MAX_JSON_BYTES: usize = 16 * 1024 * 1024;

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

fn validate_real_process_request(request: &RealProcessBenchmarkRequestV1) -> Result<()> {
    let shape = TopologyShape::new(request.participants);
    shape.validate()?;
    ensure!(
        request.version == 1
            && request.protocol == "AtomicPrivateSettlementV1"
            && request.kind == "benchmark"
            && matches!(
                request.payload.profile.as_str(),
                "private" | "transparent_control"
            ),
        "Rust harness supports only AtomicPrivateSettlementV1 benchmark profiles"
    );
    ensure!(
        request.validators_per_dataspace == VALIDATORS_PER_LANE
            && request.global_validators == VALIDATORS_PER_LANE
            && request.quorum == "3-of-4"
            && request.mandatory_signed_rs16_da_rbc
            && request.authenticated_message_control,
        "request weakens the required real-process topology"
    );
    ensure!(
        request.minimum_signed_rs16_da_observations
            == u64::try_from(shape.peer_count()).expect("peer count fits u64"),
        "signed RS16 observation minimum must cover every validator"
    );
    ensure!(
        lowercase_digest(&request.request_id, &[64])
            && lowercase_digest(&request.invocation_nonce, &[64])
            && lowercase_digest(&request.commit, &[40, 64])
            && lowercase_digest(&request.hardware_sha256, &[64])
            && lowercase_digest(&request.hardware_profile_sha256, &[64])
            && lowercase_digest(&request.configuration_sha256, &[64]),
        "request contains a malformed binding digest"
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
    ensure!(
        request
            .configuration
            .get("version")
            .and_then(HarnessJsonValue::as_u64)
            == Some(1)
            && request
                .configuration
                .get("protocol")
                .and_then(HarnessJsonValue::as_str)
                == Some("AtomicPrivateSettlementV1")
            && request
                .configuration
                .get("participants")
                .and_then(HarnessJsonValue::as_u64)
                == u64::try_from(request.participants).ok()
            && harness_json_object_u64(
                &request.configuration,
                "topology",
                "validators_per_dataspace",
            ) == Some(4)
            && harness_json_object_u64(&request.configuration, "topology", "global_validators",)
                == Some(4)
            && harness_json_object_bool(
                &request.configuration,
                "consensus",
                "mandatory_signed_rs16_da_rbc",
            ) == Some(true)
            && harness_json_object_bool(
                &request.configuration,
                "consensus",
                "authenticated_message_control",
            ) == Some(true),
        "embedded configuration does not bind the required topology"
    );
    let _ = request.payload.warmup;
    let _ = request.run;
    Ok(())
}

fn read_bound_real_process_request() -> Result<(RealProcessBenchmarkRequestV1, String)> {
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
    let request: RealProcessBenchmarkRequestV1 =
        norito::json::from_str(text).wrap_err("decode strict real-process request")?;
    validate_real_process_request(&request)?;
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
) -> Result<Vec<RealProcessInventoryRowV1>> {
    let validator_sha =
        std::env::var(HARNESS_VALIDATOR_SHA_ENV).wrap_err("missing validator executable digest")?;
    ensure!(
        lowercase_digest(&validator_sha, &[64]),
        "validator executable digest is malformed"
    );
    let coordinator_pid = std::process::id();
    let coordinator_path = std::env::current_exe().wrap_err("resolve coordinator executable")?;
    let mut rows = vec![RealProcessInventoryRowV1 {
        role: "coordinator".to_owned(),
        dataspace_ordinal: None,
        validator_ordinal: None,
        pid: coordinator_pid,
        executable_sha256: sha256_regular_file(&coordinator_path)?,
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
            let expected = headers[0];
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
    let inventory = collect_process_inventory(&network, &runtime, shape, &request.commit)?;
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
    let inventory = collect_process_inventory(&network, &runtime, shape, &request.commit)?;
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
            // These zeroes are backed by the collecting/audited/prepared/commit-certified
            // phase-boundary assertions above, not by a continuous fault observer.
            // TODO: Add the full-topology concurrent observer before enabling the
            // authenticated fault-injection benchmark branch.
            partial_visible_observations: 0,
            partial_spendable_observations: 0,
        },
    })
}

fn write_real_process_result(result: &RealProcessBenchmarkResultV1) -> Result<()> {
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
            let (request, request_sha) = read_bound_real_process_request()?;
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
fn real_process_request_digest_validation_is_fail_closed() {
    assert!(lowercase_digest(&"a".repeat(64), &[64]));
    assert!(lowercase_digest(&"b".repeat(40), &[40, 64]));
    assert!(!lowercase_digest(&"0".repeat(64), &[64]));
    assert!(!lowercase_digest(&"A".repeat(64), &[64]));
    assert!(!lowercase_digest("not-a-digest", &[64]));
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
