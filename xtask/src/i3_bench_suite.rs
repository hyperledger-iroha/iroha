use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    error::Error,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::Response,
    routing::post,
};
use eyre::eyre;
use iroha_crypto::{Algorithm, KeyPair, PublicKey, Signature};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use tower::ServiceExt;

#[derive(Clone, Debug)]
pub struct I3BenchOptions {
    pub iterations: u32,
    pub sample_size: u32,
    pub json_out: PathBuf,
    pub csv_out: Option<PathBuf>,
    pub markdown_out: Option<PathBuf>,
    pub allow_overwrite: bool,
    pub threshold: Option<PathBuf>,
    pub flamegraph_hint: bool,
}

#[derive(Clone, Debug, JsonSerialize)]
pub struct I3BenchReport {
    pub timestamp: String,
    pub git_hash: Option<String>,
    pub config: BenchConfig,
    pub scenarios: Vec<ScenarioResult>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub struct BenchConfig {
    pub iterations: u32,
    pub sample_size: u32,
    pub flamegraph_hint: bool,
}

#[derive(Clone, Debug, JsonSerialize)]
pub struct ScenarioResult {
    pub name: String,
    pub nanos_per_iter: u64,
    pub throughput_per_sec: f64,
    pub allocations_per_iter: f64,
    pub note: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct Thresholds {
    allow_slowdown_bps: u32,
    scenarios: Vec<ScenarioBound>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct ScenarioBound {
    name: String,
    max_nanos_per_iter: u64,
}

#[derive(Clone)]
struct ProofFixtures {
    commit_message: Vec<u8>,
    commit_signatures: Vec<ProofSignature>,
    attestation: ProofSignature,
    bridge: ProofSignature,
}

#[derive(Clone)]
struct ProofSignature {
    public_key: PublicKey,
    signature: Signature,
    message: Vec<u8>,
}

#[derive(Clone)]
struct SchedulerAccess {
    reads: BTreeSet<u64>,
    writes: BTreeSet<u64>,
}

#[derive(Clone)]
struct FeeLedger {
    payer: u128,
    sponsor: u128,
    collector: u128,
}

#[derive(Clone)]
struct StakeLedger {
    bonded: u128,
    pending_unbond: u128,
    slashed: u128,
}

pub fn run_i3_bench_suite(options: I3BenchOptions) -> Result<I3BenchReport, Box<dyn Error>> {
    if options.iterations == 0 {
        return Err("iterations must be greater than zero".into());
    }
    if options.sample_size == 0 {
        return Err("sample size must be greater than zero".into());
    }

    let fixtures = BenchFixtures::new();
    let scenarios = vec![
        bench_fee(
            "fee_payer",
            "payer-funded charge",
            options.iterations,
            options.sample_size,
            FeeScenario::PayerOnly,
            &fixtures.fee_ledger,
        ),
        bench_fee(
            "fee_sponsor",
            "sponsor fallback debit",
            options.iterations,
            options.sample_size,
            FeeScenario::SponsorAllowed,
            &fixtures.fee_ledger,
        ),
        bench_fee(
            "fee_insufficient",
            "insufficient fee rejection",
            options.iterations,
            options.sample_size,
            FeeScenario::Insufficient,
            &fixtures.fee_ledger,
        ),
        bench_staking(
            "staking_bond",
            "bond + withdraw queue",
            options.iterations,
            options.sample_size,
            StakingScenario::BondThenWithdraw,
            &fixtures.stake_ledger,
        ),
        bench_staking(
            "staking_slash",
            "slash before apply",
            options.iterations,
            options.sample_size,
            StakingScenario::SlashBeforeApply,
            &fixtures.stake_ledger,
        ),
        bench_proof_verify(
            "commit_cert_verify",
            "verify commit cert signatures",
            options.iterations,
            options.sample_size,
            &fixtures.proofs.commit_signatures,
        ),
        bench_single_proof(
            "jdg_attestation_verify",
            "verify JDG attestation signature",
            options.iterations,
            options.sample_size,
            &fixtures.proofs.attestation,
        ),
        bench_single_proof(
            "bridge_proof_verify",
            "verify bridge proof signature",
            options.iterations,
            options.sample_size,
            &fixtures.proofs.bridge,
        ),
        bench_commit_assembly(
            "commit_cert_assembly",
            "assemble commit cert digest",
            options.iterations,
            options.sample_size,
            &fixtures.proofs.commit_signatures,
        ),
        bench_scheduler(
            "access_scheduler",
            "derive batches from access sets",
            options.iterations,
            options.sample_size,
            &fixtures.scheduler,
        ),
        bench_proof_endpoint(
            "torii_proof_endpoint",
            "Axum proof endpoint parse + verify",
            options.iterations,
            options.sample_size,
            &fixtures.endpoint,
        )?,
    ];

    let report = I3BenchReport {
        timestamp: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)?
            .to_string(),
        git_hash: detect_git_hash(),
        config: BenchConfig {
            iterations: options.iterations,
            sample_size: options.sample_size,
            flamegraph_hint: options.flamegraph_hint,
        },
        scenarios,
    };

    write_json(&report, &options.json_out, options.allow_overwrite)?;
    if let Some(csv) = &options.csv_out {
        write_csv(&report, csv, options.allow_overwrite)?;
    }
    if let Some(md) = &options.markdown_out {
        write_markdown(&report, md, options.allow_overwrite)?;
    }
    if let Some(thresholds) = &options.threshold {
        check_thresholds(thresholds, &report)?;
    }

    Ok(report)
}

fn detect_git_hash() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
    } else {
        None
    }
}

fn bench_fee(
    name: &str,
    note: &str,
    iterations: u32,
    sample_size: u32,
    scenario: FeeScenario,
    ledger: &FeeLedger,
) -> ScenarioResult {
    bench_scenario(name, note, iterations, sample_size, || match scenario {
        FeeScenario::PayerOnly => ledger.clone().charge(75_000, false),
        FeeScenario::SponsorAllowed => ledger.clone().charge(75_000, true),
        FeeScenario::Insufficient => ledger.clone().charge(3_000_000_000, true),
    })
}

fn bench_staking(
    name: &str,
    note: &str,
    iterations: u32,
    sample_size: u32,
    scenario: StakingScenario,
    ledger: &StakeLedger,
) -> ScenarioResult {
    bench_scenario(name, note, iterations, sample_size, || {
        let mut ledger = ledger.clone();
        match scenario {
            StakingScenario::BondThenWithdraw => ledger.bond_cycle(50_000),
            StakingScenario::SlashBeforeApply => ledger.slash_cycle(12_500, 750),
        }
    })
}

fn bench_proof_verify(
    name: &str,
    note: &str,
    iterations: u32,
    sample_size: u32,
    proofs: &[ProofSignature],
) -> ScenarioResult {
    bench_scenario(name, note, iterations, sample_size, || {
        let mut allocs = 0;
        for proof in proofs {
            proof
                .signature
                .verify(&proof.public_key, &proof.message)
                .expect("deterministic proof verification");
            allocs += 1;
        }
        allocs
    })
}

fn bench_single_proof(
    name: &str,
    note: &str,
    iterations: u32,
    sample_size: u32,
    proof: &ProofSignature,
) -> ScenarioResult {
    bench_scenario(name, note, iterations, sample_size, || {
        proof
            .signature
            .verify(&proof.public_key, &proof.message)
            .expect("deterministic proof verification");
        1
    })
}

fn bench_commit_assembly(
    name: &str,
    note: &str,
    iterations: u32,
    sample_size: u32,
    proofs: &[ProofSignature],
) -> ScenarioResult {
    bench_scenario(name, note, iterations, sample_size, || {
        let mut hasher = Sha256::new();
        let mut allocs = 1;
        for proof in proofs.iter().take(12) {
            hasher.update(proof.message.as_slice());
            hasher.update(proof.signature.payload());
            allocs += 1;
        }
        let digest = hasher.finalize();
        std::hint::black_box(digest);
        allocs
    })
}

fn bench_scheduler(
    name: &str,
    note: &str,
    iterations: u32,
    sample_size: u32,
    accesses: &[SchedulerAccess],
) -> ScenarioResult {
    bench_scenario(name, note, iterations, sample_size, || {
        let mut batches: Vec<VecDeque<&SchedulerAccess>> = Vec::new();
        let mut allocs = 1;
        for access in accesses {
            if let Some(last) = batches.last_mut() {
                if conflicts(last.back().expect("non-empty batch"), access) {
                    let mut queue = VecDeque::new();
                    queue.push_back(access);
                    batches.push(queue);
                    allocs += 1;
                } else {
                    last.push_back(access);
                }
            } else {
                let mut queue = VecDeque::new();
                queue.push_back(access);
                batches.push(queue);
                allocs += 1;
            }
        }
        allocs
    })
}

fn bench_proof_endpoint(
    name: &str,
    note: &str,
    iterations: u32,
    sample_size: u32,
    endpoint: &ProofEndpointFixture,
) -> Result<ScenarioResult, Box<dyn Error>> {
    let runtime = Runtime::new()?;
    let router = endpoint.router.clone();
    let request_body = json::to_vec(&endpoint.request)?;

    Ok(bench_scenario(name, note, iterations, sample_size, || {
        let request = Request::builder()
            .uri("/proof")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(request_body.clone()))
            .expect("request");
        let resp = runtime
            .block_on(router.clone().oneshot(request))
            .expect("handler executes");
        if resp.status() != StatusCode::OK {
            panic!("unexpected status {}", resp.status());
        }
        2
    }))
}

fn bench_scenario<F>(
    name: &str,
    note: &str,
    iterations: u32,
    sample_size: u32,
    mut workload: F,
) -> ScenarioResult
where
    F: FnMut() -> u64,
{
    let mut durations = Vec::with_capacity(sample_size as usize);
    let mut allocations = 0_u64;
    for _ in 0..sample_size {
        let start = Instant::now();
        for _ in 0..iterations {
            allocations = allocations.saturating_add(workload());
        }
        durations.push(start.elapsed());
    }

    let nanos = median_duration(&mut durations).as_nanos() as u64 / iterations as u64;
    let throughput = 1_000_000_000.0 / nanos as f64;
    let total_iters = (iterations as u64) * (sample_size as u64);
    let allocs_per_iter = allocations as f64 / total_iters as f64;

    ScenarioResult {
        name: name.to_string(),
        nanos_per_iter: nanos,
        throughput_per_sec: throughput,
        allocations_per_iter: allocs_per_iter,
        note: note.to_string(),
    }
}

fn median_duration(values: &mut [Duration]) -> Duration {
    values.sort_unstable();
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        values[mid - 1].saturating_add(values[mid]) / 2
    } else {
        values[mid]
    }
}

fn write_json(
    report: &I3BenchReport,
    path: &Path,
    allow_overwrite: bool,
) -> Result<(), eyre::Error> {
    if path.exists() && !allow_overwrite {
        return Err(eyre!("refusing to overwrite existing {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = json::to_vec_pretty(report)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn write_csv(
    report: &I3BenchReport,
    path: &Path,
    allow_overwrite: bool,
) -> Result<(), eyre::Error> {
    if path.exists() && !allow_overwrite {
        return Err(eyre!("refusing to overwrite existing {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = String::new();
    writeln!(
        out,
        "name,nanos_per_iter,throughput_per_sec,allocations_per_iter,note"
    )?;
    for scenario in &report.scenarios {
        writeln!(
            out,
            "{},{},{:.4},{:.3},\"{}\"",
            scenario.name,
            scenario.nanos_per_iter,
            scenario.throughput_per_sec,
            scenario.allocations_per_iter,
            scenario.note.replace('\"', "'")
        )?;
    }
    fs::write(path, out)?;
    Ok(())
}

fn write_markdown(
    report: &I3BenchReport,
    path: &Path,
    allow_overwrite: bool,
) -> Result<(), eyre::Error> {
    if path.exists() && !allow_overwrite {
        return Err(eyre!("refusing to overwrite existing {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = String::new();
    writeln!(
        out,
        "# Iroha 3 bench suite\n\n- timestamp: {}\n- git hash: {}\n- iterations: {}\n- sample count: {}\n",
        report.timestamp,
        report.git_hash.as_deref().unwrap_or("unknown"),
        report.config.iterations,
        report.config.sample_size
    )?;
    writeln!(
        out,
        "| scenario | ns/iter | throughput/sec | allocations/iter | note |"
    )?;
    writeln!(out, "| --- | --- | --- | --- | --- |")?;
    for scenario in &report.scenarios {
        writeln!(
            out,
            "| {} | {} | {:.2} | {:.2} | {} |",
            scenario.name,
            scenario.nanos_per_iter,
            scenario.throughput_per_sec,
            scenario.allocations_per_iter,
            scenario.note
        )?;
    }
    if report.config.flamegraph_hint {
        writeln!(
            out,
            "\nFlamegraph hint: re-run a scenario under `cargo flamegraph -p xtask -- i3-bench-suite --iterations {}` to capture profile samples.",
            report.config.iterations
        )?;
    }
    fs::write(path, out)?;
    Ok(())
}

fn check_thresholds(thresholds: &Path, report: &I3BenchReport) -> Result<(), eyre::Error> {
    let data = fs::read(thresholds)?;
    let thresholds: Thresholds = json::from_slice(&data)?;
    let mut bounds = HashMap::new();
    for scenario in thresholds.scenarios {
        bounds.insert(scenario.name, scenario.max_nanos_per_iter);
    }
    for scenario in &report.scenarios {
        if let Some(limit) = bounds.get(&scenario.name)
            && scenario.nanos_per_iter > *limit
        {
            return Err(eyre!(
                "scenario {} exceeded bound: {} ns/iter > {} ns/iter",
                scenario.name,
                scenario.nanos_per_iter,
                limit
            ));
        }
    }
    if thresholds.allow_slowdown_bps > 0 {
        let tolerance = thresholds.allow_slowdown_bps as f64 / 10_000.0;
        if let Some(limit) = bounds.get("torii_proof_endpoint") {
            for scenario in &report.scenarios {
                if let Some(base) = bounds.get(&scenario.name) {
                    let allowed = (*base as f64) * (1.0 + tolerance);
                    if (scenario.nanos_per_iter as f64) > allowed
                        && scenario.name != "torii_proof_endpoint"
                    {
                        return Err(eyre!(
                            "scenario {} exceeded tolerance ({} ns/iter over allowed {:.0})",
                            scenario.name,
                            scenario.nanos_per_iter,
                            allowed
                        ));
                    }
                }
            }
            if let Some(endpoint) = report
                .scenarios
                .iter()
                .find(|s| s.name == "torii_proof_endpoint")
                && endpoint.nanos_per_iter as f64 > (*limit as f64) * (1.0 + tolerance)
            {
                return Err(eyre!(
                    "torii_proof_endpoint exceeded bound with tolerance: {} ns/iter",
                    endpoint.nanos_per_iter
                ));
            }
        }
    }
    Ok(())
}

fn conflicts(left: &SchedulerAccess, right: &SchedulerAccess) -> bool {
    left.writes.intersection(&right.reads).next().is_some()
        || right.writes.intersection(&left.reads).next().is_some()
        || left.writes.intersection(&right.writes).next().is_some()
}

enum FeeScenario {
    PayerOnly,
    SponsorAllowed,
    Insufficient,
}

impl FeeLedger {
    fn charge(mut self, fee: u128, allow_sponsor: bool) -> u64 {
        let allocs = 1;
        if self.payer >= fee {
            self.payer -= fee;
            self.collector += fee;
            return allocs;
        }
        if allow_sponsor && self.sponsor >= fee {
            self.sponsor -= fee;
            self.collector += fee;
            return allocs + 1;
        }
        allocs + 1
    }
}

enum StakingScenario {
    BondThenWithdraw,
    SlashBeforeApply,
}

impl StakeLedger {
    fn bond_cycle(&mut self, amount: u128) -> u64 {
        let mut allocs = 1;
        self.bonded = self.bonded.saturating_add(amount);
        self.pending_unbond = self.pending_unbond.saturating_add(amount / 2);
        self.bonded = self.bonded.saturating_sub(self.pending_unbond);
        allocs += 1;
        allocs
    }

    fn slash_cycle(&mut self, amount: u128, bps: u16) -> u64 {
        let mut allocs = 1;
        self.bonded = self.bonded.saturating_add(amount);
        let slash = (self.bonded * u128::from(bps)) / 10_000;
        self.bonded = self.bonded.saturating_sub(slash);
        self.slashed = self.slashed.saturating_add(slash);
        allocs += 1;
        allocs
    }
}

struct BenchFixtures {
    fee_ledger: FeeLedger,
    stake_ledger: StakeLedger,
    proofs: ProofFixtures,
    scheduler: Vec<SchedulerAccess>,
    endpoint: ProofEndpointFixture,
}

impl BenchFixtures {
    fn new() -> Self {
        let mut rng = ChaCha20Rng::from_seed([0x42; 32]);
        let fee_ledger = FeeLedger {
            payer: 1_000_000,
            sponsor: 2_000_000,
            collector: 0,
        };
        let stake_ledger = StakeLedger {
            bonded: 500_000,
            pending_unbond: 25_000,
            slashed: 0,
        };
        let proofs = ProofFixtures::new(&mut rng);
        let scheduler = scheduler_fixtures(&mut rng);
        let endpoint = ProofEndpointFixture::new(&proofs);

        Self {
            fee_ledger,
            stake_ledger,
            proofs,
            scheduler,
            endpoint,
        }
    }
}

impl ProofFixtures {
    fn new(rng: &mut ChaCha20Rng) -> Self {
        let commit_message = random_bytes(rng, 96);
        let attestation_msg = random_bytes(rng, 512);
        let bridge_msg = random_bytes(rng, 256);

        let commit_signatures = (0..16)
            .map(|idx| {
                let mut seed = [0_u8; 32];
                seed[0] = idx;
                let pair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
                let signature = Signature::new(pair.private_key(), &commit_message);
                ProofSignature {
                    public_key: pair.public_key().clone(),
                    signature,
                    message: commit_message.clone(),
                }
            })
            .collect();

        let attestation_pair = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
        let bridge_pair = KeyPair::from_seed(vec![0xB4; 32], Algorithm::Ed25519);

        let attestation = ProofSignature {
            public_key: attestation_pair.public_key().clone(),
            signature: Signature::new(attestation_pair.private_key(), &attestation_msg),
            message: attestation_msg,
        };
        let bridge = ProofSignature {
            public_key: bridge_pair.public_key().clone(),
            signature: Signature::new(bridge_pair.private_key(), &bridge_msg),
            message: bridge_msg,
        };

        Self {
            commit_message,
            commit_signatures,
            attestation,
            bridge,
        }
    }
}

fn random_bytes(rng: &mut ChaCha20Rng, len: usize) -> Vec<u8> {
    let mut out = vec![0_u8; len];
    rng.fill_bytes(&mut out[..]);
    out
}

fn scheduler_fixtures(rng: &mut ChaCha20Rng) -> Vec<SchedulerAccess> {
    let mut fixtures = Vec::with_capacity(12);
    for lane in 0_u64..12 {
        let mut reads = BTreeSet::new();
        let mut writes = BTreeSet::new();
        reads.insert(lane);
        reads.insert(lane + 1);
        if lane % 3 == 0 {
            writes.insert(lane + 2);
        } else {
            writes.insert(lane);
        }
        if rng.next_u32().is_multiple_of(2) {
            writes.insert(lane + 4);
        }
        fixtures.push(SchedulerAccess { reads, writes });
    }
    fixtures
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
struct ProofRequest {
    signature_hex: String,
    payload_hex: String,
    version: String,
}

#[derive(Clone)]
struct ProofEndpointFixture {
    router: Router,
    request: ProofRequest,
}

impl ProofEndpointFixture {
    fn new(fixtures: &ProofFixtures) -> Self {
        let signer = fixtures
            .commit_signatures
            .first()
            .expect("at least one signature");
        let version = "v1".to_string();
        let request = ProofRequest {
            signature_hex: hex::encode_upper(signer.signature.payload()),
            payload_hex: hex::encode_upper(&fixtures.commit_message),
            version: version.clone(),
        };

        let router = Router::new()
            .route(
                "/proof",
                post(
                    |State(signature): State<ProofSignature>, body: axum::body::Bytes| async move {
                        proof_handler(signature, body).await
                    },
                ),
            )
            .with_state(signer.clone());

        Self { router, request }
    }
}

async fn proof_handler(
    signature: ProofSignature,
    body: axum::body::Bytes,
) -> Result<Response, StatusCode> {
    let request: ProofRequest = json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    if request.version != "v1" {
        return Err(StatusCode::NOT_ACCEPTABLE);
    }
    let payload = hex::decode(&request.payload_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let sig_bytes = hex::decode(&request.signature_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let candidate = Signature::from_bytes(&sig_bytes);
    candidate
        .verify(&signature.public_key, &payload)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let response_bytes = json::to_vec(&request).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(response_bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn bench_suite_smoke() {
        let temp = TempDir::new().expect("temp dir");
        let json_out = temp.path().join("bench.json");
        let report = run_i3_bench_suite(I3BenchOptions {
            iterations: 3,
            sample_size: 2,
            json_out: json_out.clone(),
            csv_out: None,
            markdown_out: None,
            allow_overwrite: true,
            threshold: None,
            flamegraph_hint: false,
        })
        .expect("bench suite runs");
        assert_eq!(report.scenarios.len(), 11);
        assert!(json_out.exists());
        assert!(
            report
                .scenarios
                .iter()
                .all(|s| s.nanos_per_iter > 0 && s.throughput_per_sec > 0.0)
        );
    }

    #[test]
    fn thresholds_detect_regression() {
        let temp = TempDir::new().expect("temp dir");
        let thresholds_path = temp.path().join("thresholds.json");
        let thresholds = Thresholds {
            allow_slowdown_bps: 0,
            scenarios: vec![ScenarioBound {
                name: "fee_payer".to_string(),
                max_nanos_per_iter: 1,
            }],
        };
        let bytes = json::to_vec(&thresholds).expect("serialize thresholds");
        fs::write(&thresholds_path, bytes).expect("write thresholds");

        let report = I3BenchReport {
            timestamp: "now".to_string(),
            git_hash: None,
            config: BenchConfig {
                iterations: 1,
                sample_size: 1,
                flamegraph_hint: false,
            },
            scenarios: vec![ScenarioResult {
                name: "fee_payer".to_string(),
                nanos_per_iter: 5,
                throughput_per_sec: 1.0,
                allocations_per_iter: 1.0,
                note: "test".to_string(),
            }],
        };

        let err = check_thresholds(&thresholds_path, &report).expect_err("should fail");
        assert!(err.to_string().contains("exceeded bound"));
    }

    #[test]
    fn bench_scenario_records_allocations() {
        let result = bench_scenario("alloc", "counts", 4, 2, || 2);
        assert_eq!(result.allocations_per_iter, 2.0);
        assert!(result.nanos_per_iter > 0);
    }
}
