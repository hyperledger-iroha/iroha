//! Validate `SoraNet` handshake performance against reference tolerances.

use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use iroha_crypto::{
    Algorithm, KeyPair,
    soranet::handshake::{
        DEFAULT_DESCRIPTOR_COMMIT, HandshakeSuite, HarnessError, RuntimeParams, build_client_hello,
        client_handle_relay_hello, relay_finalize_handshake,
    },
};
use norito::json::{self, Map, Value};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

const DEFAULT_BASELINE: &str = "crates/iroha_crypto/benches/soranet_handshake_baseline.json";
const DEFAULT_TOLERANCE: f64 = 0.15;
const MAX_P99_NS: u128 = 900_000_000; // 900ms
const DEFAULT_SAMPLES: usize = 200;

#[derive(Clone)]
struct HandshakeScenario {
    client_caps: Vec<u8>,
    relay_caps: Vec<u8>,
}

impl HandshakeScenario {
    fn new(preferred: HandshakeSuite) -> Self {
        let (client_caps, relay_caps) = match preferred {
            HandshakeSuite::Nk2Hybrid => load_nk2_caps(),
            HandshakeSuite::Nk3PqForwardSecure => load_nk3_caps(),
        };
        Self {
            client_caps,
            relay_caps,
        }
    }

    fn params(&self) -> RuntimeParams<'_> {
        RuntimeParams {
            descriptor_commit: &DEFAULT_DESCRIPTOR_COMMIT,
            client_capabilities: &self.client_caps,
            relay_capabilities: &self.relay_caps,
            kem_id: 1,
            sig_id: 1,
            resume_hash: None,
        }
    }
}

fn run_handshake(suite: HandshakeSuite) -> Result<(), HarnessError> {
    let scenario = HandshakeScenario::new(suite);
    let params = scenario.params();

    let mut rng_client = ChaCha20Rng::from_seed([0xA5; 32]);
    let mut rng_relay = ChaCha20Rng::from_seed([0x5A; 32]);
    let client_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let relay_keys = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);

    let (client_hello, client_state) = build_client_hello(&params, &mut rng_client)?;
    let client_hello_len = client_hello.len();
    let (relay_message, relay_state) = iroha_crypto::soranet::handshake::process_client_hello(
        &client_hello,
        &params,
        &relay_keys,
        &mut rng_relay,
    )
    .inspect_err(|_err| {
        eprintln!(
            "process_client_hello failed for suite {:?}\nclient_caps_len={} relay_caps_len={} client_hello_len={}\nclient_caps={}\nrelay_caps={}",
            suite,
            scenario.client_caps.len(),
            scenario.relay_caps.len(),
            client_hello_len,
            hex::encode(&scenario.client_caps),
            hex::encode(&scenario.relay_caps)
        );
    })?;
    let (client_finish, _) = client_handle_relay_hello(
        client_state,
        &relay_message,
        &client_keys,
        &params,
        &mut rng_client,
    )?;

    let finish = client_finish.as_deref().unwrap_or(&[]);
    relay_finalize_handshake(relay_state, finish, &relay_keys)?;
    Ok(())
}

fn measure_suite(suite: HandshakeSuite, samples: usize) -> Result<Vec<u128>, HarnessError> {
    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        run_handshake(suite)?;
        timings.push(start.elapsed().as_nanos());
    }
    Ok(timings)
}

fn mean_ns(samples: &[u128]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: u128 = samples.iter().copied().sum();
    let len = u128::try_from(samples.len()).expect("sample count fits u128");
    let quotient = sum / len;
    let remainder = sum % len;
    let quotient_f64 = u128_to_f64(quotient);
    let remainder_f64 = u128_to_f64(remainder);
    let len_f64 = u128_to_f64(len);
    quotient_f64 + (remainder_f64 / len_f64)
}

fn u128_to_f64(value: u128) -> f64 {
    debug_assert!(value <= (1u128 << f64::MANTISSA_DIGITS));
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

fn usize_to_f64(value: usize) -> f64 {
    debug_assert!(value <= (1usize << f64::MANTISSA_DIGITS));
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

fn f64_to_usize(value: f64) -> usize {
    debug_assert!(value >= 0.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        value as usize
    }
}

fn percentile(samples: &mut [u128], percentile: f64) -> u128 {
    samples.sort_unstable();
    let len_f64 = usize_to_f64(samples.len());
    let rank = (len_f64 * percentile).ceil().clamp(1.0, len_f64);
    let index = f64_to_usize(rank).saturating_sub(1);
    samples[index.min(samples.len().saturating_sub(1))]
}

fn decode_hex_vec(label: &str, hex_str: &str) -> Vec<u8> {
    hex::decode(hex_str).unwrap_or_else(|err| panic!("{label} hex decode failed: {err}"))
}

fn load_nk2_caps() -> (Vec<u8>, Vec<u8>) {
    let raw =
        include_str!("../../../../tests/interop/soranet/interop/rust/snnet-interop-nk2-v1.json");
    let value: Value = norito::json::from_str(raw).expect("parse nk2 interop fixture");
    let inputs = value["inputs"].as_object().expect("nk2 inputs object");
    let client_hex = inputs["client_capabilities_hex"]
        .as_str()
        .expect("nk2 client capabilities hex");
    let relay_hex = inputs["relay_capabilities_hex"]
        .as_str()
        .expect("nk2 relay capabilities hex");
    (
        decode_hex_vec("nk2 client", client_hex),
        decode_hex_vec("nk2 relay", relay_hex),
    )
}

fn load_nk3_caps() -> (Vec<u8>, Vec<u8>) {
    let raw =
        include_str!("../../../../tests/interop/soranet/interop/rust/snnet-interop-nk3-v1.json");
    let value: Value = norito::json::from_str(raw).expect("parse nk3 interop fixture");
    let inputs = value["inputs"].as_object().expect("nk3 inputs object");
    let client_hex = inputs["client_capabilities_hex"]
        .as_str()
        .expect("nk3 client capabilities hex");
    let relay_hex = inputs["relay_capabilities_hex"]
        .as_str()
        .expect("nk3 relay capabilities hex");
    (
        decode_hex_vec("nk3 client", client_hex),
        decode_hex_vec("nk3 relay", relay_hex),
    )
}

fn load_baseline(path: &Path) -> Result<BTreeMap<String, BaselineEntry>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let value: Value = json::from_str(&text)?;
    let mut map = BTreeMap::new();
    if let Value::Object(entries) = value {
        for (key, entry) in entries {
            if let Value::Object(obj) = entry {
                let mean = obj
                    .get("mean_ns")
                    .and_then(Value::as_f64)
                    .ok_or("baseline missing mean_ns")?;
                let p99 = obj
                    .get("p99_ns")
                    .and_then(Value::as_f64)
                    .ok_or("baseline missing p99_ns")?;
                map.insert(
                    key,
                    BaselineEntry {
                        mean_ns: mean,
                        p99_ns: p99,
                    },
                );
            }
        }
    }
    Ok(map)
}

fn write_baseline(
    path: &Path,
    samples: &BTreeMap<&'static str, Metrics>,
) -> Result<(), Box<dyn Error>> {
    let mut root = Map::new();
    for (&label, metrics) in samples {
        let mut entry = Map::new();
        entry.insert("mean_ns".to_string(), Value::from(metrics.mean_ns));
        entry.insert("p99_ns".to_string(), Value::from(metrics.p99_ns));
        root.insert(label.to_string(), Value::Object(entry));
    }
    let text = json::to_string_pretty(&Value::Object(root))?;
    fs::write(path, text)?;
    Ok(())
}

struct Metrics {
    mean_ns: f64,
    p99_ns: f64,
}

struct BaselineEntry {
    mean_ns: f64,
    p99_ns: f64,
}

struct Cli {
    baseline: PathBuf,
    tolerance: f64,
    samples: usize,
    write_baseline: Option<PathBuf>,
}

fn parse_args() -> Result<Cli, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let mut baseline = PathBuf::from(DEFAULT_BASELINE);
    let mut tolerance = DEFAULT_TOLERANCE;
    let mut samples = DEFAULT_SAMPLES;
    let mut write_baseline = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--baseline" => {
                let value = args.next().ok_or("missing value for --baseline")?;
                baseline = PathBuf::from(value);
            }
            "--tolerance" => {
                let value = args.next().ok_or("missing value for --tolerance")?;
                tolerance = value.parse()?;
            }
            "--samples" => {
                let value = args.next().ok_or("missing value for --samples")?;
                samples = value.parse()?;
            }
            "--write-baseline" => {
                let value = args.next().ok_or("missing value for --write-baseline")?;
                write_baseline = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                println!("Usage: soranet_handshake_check [OPTIONS]");
                println!(
                    "  --baseline <path>          Path to baseline JSON (default: {DEFAULT_BASELINE})"
                );
                println!(
                    "  --tolerance <decimal>      Allowed regression ratio (default: {DEFAULT_TOLERANCE})"
                );
                println!(
                    "  --samples <count>          Number of measurements per suite (default: {DEFAULT_SAMPLES})"
                );
                println!("  --write-baseline <path>    Write freshly measured baseline and exit");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}").into()),
        }
    }

    Ok(Cli {
        baseline,
        tolerance,
        samples,
        write_baseline,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = parse_args()?;

    let suites = [
        ("nk2_hybrid", HandshakeSuite::Nk2Hybrid),
        ("nk3_forward_secure", HandshakeSuite::Nk3PqForwardSecure),
    ];

    let mut metrics = BTreeMap::new();
    for (label, suite) in suites {
        let mut samples = measure_suite(suite, cli.samples)
            .map_err(|err| format!("handshake {label} failed during measurement: {err}"))?;
        let mean = mean_ns(&samples);
        let p99 = percentile(&mut samples, 0.99);
        metrics.insert(
            label,
            Metrics {
                mean_ns: mean,
                p99_ns: u128_to_f64(p99),
            },
        );
        println!(
            "{label}: mean {:.3}µs, p99 {:.3}µs",
            mean / 1_000.0,
            u128_to_f64(p99) / 1_000.0
        );
        if p99 > MAX_P99_NS {
            return Err(format!(
                "{label} p99 {:.3}ms exceeds 900ms limit",
                u128_to_f64(p99) / 1_000_000.0
            )
            .into());
        }
    }

    if let Some(path) = cli.write_baseline.as_deref() {
        write_baseline(path, &metrics)?;
        println!("baseline written to {}", path.display());
        return Ok(());
    }

    let baseline = load_baseline(&cli.baseline)?;
    for (label, data) in &metrics {
        if let Some(reference) = baseline.get(*label) {
            if data.mean_ns > reference.mean_ns * (1.0 + cli.tolerance) {
                return Err(format!(
                    "{label} regression: measured {:.3}µs vs baseline {:.3}µs (+{:.1}%)",
                    data.mean_ns / 1_000.0,
                    reference.mean_ns / 1_000.0,
                    ((data.mean_ns / reference.mean_ns) - 1.0) * 100.0
                )
                .into());
            }
            if data.p99_ns > reference.p99_ns * (1.0 + cli.tolerance) {
                return Err(format!(
                    "{label} p99 regression: measured {:.3}µs vs baseline {:.3}µs (+{:.1}%)",
                    data.p99_ns / 1_000.0,
                    reference.p99_ns / 1_000.0,
                    ((data.p99_ns / reference.p99_ns) - 1.0) * 100.0
                )
                .into());
            }
        } else {
            println!("warning: no baseline entry for {label}; skipping regression check");
        }
    }

    Ok(())
}
