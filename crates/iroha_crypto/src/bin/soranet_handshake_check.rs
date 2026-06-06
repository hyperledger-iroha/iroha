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
const SUITES: [(&str, HandshakeSuite); 2] = [
    ("nk2_hybrid", HandshakeSuite::Nk2Hybrid),
    ("nk3_forward_secure", HandshakeSuite::Nk3PqForwardSecure),
];

#[derive(Clone)]
struct HandshakeScenario {
    client_caps: Vec<u8>,
    relay_caps: Vec<u8>,
}

impl HandshakeScenario {
    fn new(preferred: HandshakeSuite) -> Result<Self, HarnessError> {
        let (client_caps, relay_caps) = match preferred {
            HandshakeSuite::Nk2Hybrid => load_nk2_caps()?,
            HandshakeSuite::Nk3PqForwardSecure => load_nk3_caps()?,
        };
        Ok(Self {
            client_caps,
            relay_caps,
        })
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
    let scenario = HandshakeScenario::new(suite)?;
    let params = scenario.params();

    let mut rng_client = ChaCha20Rng::from_seed([0xA5; 32]);
    let mut rng_relay = ChaCha20Rng::from_seed([0x5A; 32]);
    let client_keys = fixed_ed25519_keypair("client", 0x11)?;
    let relay_keys = fixed_ed25519_keypair("relay", 0x22)?;

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

fn fixed_ed25519_keypair(label: &str, seed_byte: u8) -> Result<KeyPair, HarnessError> {
    KeyPair::try_from_seed(vec![seed_byte; 32], Algorithm::Ed25519).map_err(|err| {
        HarnessError::Validation(format!(
            "failed to derive {label} handshake-check Ed25519 keypair: {err}"
        ))
    })
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

fn decode_hex_vec(label: &str, hex_str: &str) -> Result<Vec<u8>, HarnessError> {
    hex::decode(hex_str)
        .map_err(|err| HarnessError::Validation(format!("{label} hex decode failed: {err}")))
}

fn load_caps_from_fixture(fixture: &str, raw: &str) -> Result<(Vec<u8>, Vec<u8>), HarnessError> {
    let value: Value = norito::json::from_str(raw).map_err(|err| {
        HarnessError::Validation(format!("parse {fixture} interop fixture: {err}"))
    })?;
    let inputs = value
        .as_object()
        .and_then(|root| root.get("inputs"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            HarnessError::Validation(format!(
                "{fixture} interop fixture is missing inputs object"
            ))
        })?;
    let client_hex = fixture_string_field(inputs, fixture, "client_capabilities_hex")?;
    let relay_hex = fixture_string_field(inputs, fixture, "relay_capabilities_hex")?;
    Ok((
        decode_hex_vec(&format!("{fixture} client"), client_hex)?,
        decode_hex_vec(&format!("{fixture} relay"), relay_hex)?,
    ))
}

fn fixture_string_field<'a>(
    inputs: &'a Map,
    fixture: &str,
    field: &str,
) -> Result<&'a str, HarnessError> {
    inputs.get(field).and_then(Value::as_str).ok_or_else(|| {
        HarnessError::Validation(format!(
            "{fixture} interop fixture field `{field}` must be a string"
        ))
    })
}

fn load_nk2_caps() -> Result<(Vec<u8>, Vec<u8>), HarnessError> {
    let raw =
        include_str!("../../../../tests/interop/soranet/interop/rust/snnet-interop-nk2-v1.json");
    load_caps_from_fixture("nk2", raw)
}

fn load_nk3_caps() -> Result<(Vec<u8>, Vec<u8>), HarnessError> {
    let raw =
        include_str!("../../../../tests/interop/soranet/interop/rust/snnet-interop-nk3-v1.json");
    load_caps_from_fixture("nk3", raw)
}

fn load_baseline(path: &Path) -> Result<BTreeMap<String, BaselineEntry>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    parse_baseline(&text)
}

fn parse_baseline(text: &str) -> Result<BTreeMap<String, BaselineEntry>, Box<dyn Error>> {
    let value: Value = json::from_str(text)?;
    let mut map = BTreeMap::new();
    let Value::Object(entries) = value else {
        return Err("baseline root must be an object".into());
    };
    for (key, entry) in entries {
        let Value::Object(obj) = entry else {
            return Err(format!("baseline entry `{key}` must be an object").into());
        };
        let mean = baseline_metric(&obj, &key, "mean_ns")?;
        let p99 = baseline_metric(&obj, &key, "p99_ns")?;
        map.insert(
            key,
            BaselineEntry {
                mean_ns: mean,
                p99_ns: p99,
            },
        );
    }
    Ok(map)
}

fn validate_baseline_coverage(
    baseline: &BTreeMap<String, BaselineEntry>,
    suites: &[(&str, HandshakeSuite)],
) -> Result<(), Box<dyn Error>> {
    for &(label, _) in suites {
        if !baseline.contains_key(label) {
            return Err(format!("baseline missing required entry `{label}`").into());
        }
    }

    for key in baseline.keys() {
        if !suites.iter().any(|(label, _)| label == key) {
            return Err(
                format!("baseline entry `{key}` is not an active SoraNet handshake suite").into(),
            );
        }
    }

    Ok(())
}

fn baseline_metric(obj: &Map, label: &str, field: &str) -> Result<f64, Box<dyn Error>> {
    let value = obj
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("baseline entry `{label}` missing {field}"))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("baseline entry `{label}` {field} must be finite and positive").into());
    }
    Ok(value)
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

#[derive(Debug)]
struct Cli {
    baseline: PathBuf,
    tolerance: f64,
    samples: usize,
    write_baseline: Option<PathBuf>,
}

fn parse_args_from<I, S>(args: I) -> Result<Cli, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
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

    if samples == 0 {
        return Err("--samples must be greater than zero".into());
    }
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err("--tolerance must be a finite non-negative decimal".into());
    }

    Ok(Cli {
        baseline,
        tolerance,
        samples,
        write_baseline,
    })
}

fn parse_args() -> Result<Cli, Box<dyn Error>> {
    parse_args_from(std::env::args().skip(1))
}

fn validate_performance_build_profile() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Err(
            "soranet_handshake_check must be run with a release profile for baseline comparison"
                .into(),
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = parse_args()?;
    validate_performance_build_profile()?;

    let mut metrics = BTreeMap::new();
    for (label, suite) in SUITES {
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
    validate_baseline_coverage(&baseline, &SUITES)?;
    for (label, data) in &metrics {
        let reference = baseline
            .get(*label)
            .ok_or_else(|| format!("baseline missing required entry `{label}`"))?;
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
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_ed25519_keypair_uses_checked_seed_derivation() {
        let keypair = fixed_ed25519_keypair("client", 0x11)
            .expect("fixed handshake-check Ed25519 key must derive");

        assert_eq!(
            keypair
                .public_key()
                .try_algorithm()
                .expect("fixed public key algorithm"),
            Algorithm::Ed25519
        );
    }

    #[test]
    fn malformed_fixture_hex_reports_validation_error() {
        let raw =
            r#"{"inputs":{"client_capabilities_hex":"not-hex","relay_capabilities_hex":"00"}}"#;
        let err = load_caps_from_fixture("fixture", raw)
            .expect_err("malformed fixture hex must fail without panicking");

        assert!(
            err.to_string().contains("fixture client hex decode failed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_args_rejects_zero_samples() {
        let err = parse_args_from(["--samples", "0"]).expect_err("zero samples must be rejected");

        assert!(
            err.to_string().contains("greater than zero"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_args_rejects_invalid_tolerance() {
        let err = parse_args_from(["--tolerance", "NaN"])
            .expect_err("non-finite tolerance must be rejected");

        assert!(
            err.to_string().contains("finite non-negative"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_baseline_rejects_non_object_root() {
        let Err(err) = parse_baseline("[]") else {
            panic!("non-object baseline root must be rejected");
        };

        assert!(
            err.to_string().contains("root must be an object"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_baseline_rejects_non_positive_metrics() {
        let raw = r#"{"nk2_hybrid":{"mean_ns":0,"p99_ns":1}}"#;
        let Err(err) = parse_baseline(raw) else {
            panic!("non-positive baseline metrics must be rejected");
        };

        assert!(
            err.to_string()
                .contains("mean_ns must be finite and positive"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_baseline_coverage_rejects_missing_required_suite() {
        let baseline =
            parse_baseline(r#"{"nk2_hybrid":{"mean_ns":1,"p99_ns":1}}"#).expect("baseline");
        let Err(err) = validate_baseline_coverage(&baseline, &SUITES) else {
            panic!("missing active suite baseline must be rejected");
        };

        assert!(
            err.to_string()
                .contains("missing required entry `nk3_forward_secure`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_baseline_coverage_rejects_unknown_suite() {
        let baseline = parse_baseline(
            r#"{
                "nk2_hybrid":{"mean_ns":1,"p99_ns":1},
                "nk3_forward_secure":{"mean_ns":1,"p99_ns":1},
                "retired_suite":{"mean_ns":1,"p99_ns":1}
            }"#,
        )
        .expect("baseline");
        let Err(err) = validate_baseline_coverage(&baseline, &SUITES) else {
            panic!("unknown baseline suite must be rejected");
        };

        assert!(
            err.to_string()
                .contains("not an active SoraNet handshake suite"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn performance_gate_rejects_debug_profile() {
        let result = validate_performance_build_profile();
        if cfg!(debug_assertions) {
            let err = result.expect_err("debug performance comparison must fail closed");
            assert!(
                err.to_string().contains("release profile"),
                "unexpected error: {err}"
            );
        } else {
            result.expect("release performance comparison profile must be accepted");
        }
    }
}
