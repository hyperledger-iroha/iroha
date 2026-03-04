use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use norito::derive::JsonDeserialize;
use soranet_handshake_harness::{HandshakeSuite, SimulationParams, decode_hex, simulate_handshake};

#[derive(JsonDeserialize)]
struct InteropInputs {
    client_capabilities_hex: String,
    relay_capabilities_hex: String,
    client_static_sk_hex: String,
    relay_static_sk_hex: String,
    client_nonce_hex: String,
    relay_nonce_hex: String,
    descriptor_commit_hex: String,
    resume_hash_hex: Option<String>,
}

#[derive(JsonDeserialize)]
struct InteropFixture {
    suite: String,
    kem_id: u8,
    sig_id: u8,
    inputs: InteropInputs,
}

struct PerfFixture {
    suite: HandshakeSuite,
    kem_id: u8,
    sig_id: u8,
    client_capabilities: Vec<u8>,
    relay_capabilities: Vec<u8>,
    client_static_sk: [u8; 32],
    relay_static_sk: [u8; 32],
    client_nonce: [u8; 32],
    relay_nonce: [u8; 32],
    descriptor_commit: [u8; 32],
    resume_hash: Option<Vec<u8>>,
}

impl PerfFixture {
    fn params(&self) -> SimulationParams<'_> {
        SimulationParams {
            client_capabilities: self.client_capabilities.as_slice(),
            relay_capabilities: self.relay_capabilities.as_slice(),
            client_static_sk: &self.client_static_sk,
            relay_static_sk: &self.relay_static_sk,
            resume_hash: self.resume_hash.as_deref(),
            descriptor_commit: &self.descriptor_commit,
            client_nonce: &self.client_nonce,
            relay_nonce: &self.relay_nonce,
            kem_id: self.kem_id,
            sig_id: self.sig_id,
        }
    }
}

#[test]
fn handshake_perf_gate() {
    let nk2 = load_fixture("snnet-interop-nk2-v1");
    let nk3 = load_fixture("snnet-interop-nk3-v1");

    let mut nk2_durations = measure_fixture(&nk2, 128);
    let mut nk3_durations = measure_fixture(&nk3, 128);

    let nk2_p99 = percentile(&mut nk2_durations, 0.99);
    let nk3_p99 = percentile(&mut nk3_durations, 0.99);

    assert!(
        nk2_p99 < Duration::from_millis(900),
        "NK2 handshake P99 {nk2_p99:?} exceeds 900 ms limit"
    );
    assert!(
        nk3_p99 < Duration::from_millis(900),
        "NK3 handshake P99 {nk3_p99:?} exceeds 900 ms limit"
    );

    let nk2_avg = average_ms(&nk2_durations);
    let nk3_avg = average_ms(&nk3_durations);
    let ratio_limit = if cfg!(debug_assertions) {
        // Debug builds carry heavy instrumentation overhead which skews NK3 more
        // than NK2. Allow a wider envelope here; release builds keep the 15% gate.
        1.35
    } else {
        1.15
    };
    assert!(
        nk3_avg <= nk2_avg * ratio_limit,
        "NK3 average {nk3_avg:.3} ms exceeds {ratio_limit:.2}× regression over NK2 {nk2_avg:.3} ms"
    );
}

fn measure_fixture(fixture: &PerfFixture, iterations: usize) -> Vec<Duration> {
    let mut durations = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let params = fixture.params();
        let start = Instant::now();
        let result = simulate_handshake(&params).expect("simulate handshake");
        assert_eq!(
            result.handshake_suite, fixture.suite,
            "fixture suite mismatch"
        );
        durations.push(start.elapsed());
    }
    durations
}

fn percentile(samples: &mut [Duration], pct: f64) -> Duration {
    samples.sort_unstable();
    let idx = ((samples.len() as f64 * pct).ceil() as usize).saturating_sub(1);
    samples[idx.min(samples.len().saturating_sub(1))]
}

fn average_ms(samples: &[Duration]) -> f64 {
    let total_ms: f64 = samples
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .sum::<f64>();
    total_ms / samples.len() as f64
}

fn load_fixture(id: &str) -> PerfFixture {
    let mut path = workspace_root();
    path.push("fixtures");
    path.push("soranet_handshake");
    path.push("interop");
    path.push("rust");
    path.push(format!("{id}.json"));
    let contents = fs::read_to_string(path).expect("read interop fixture");
    let parsed: InteropFixture = norito::json::from_str(&contents).expect("parse interop fixture");

    let suite = match parsed.suite.as_str() {
        "nk2.hybrid" => HandshakeSuite::Nk2Hybrid,
        "nk3.pq_forward_secure" => HandshakeSuite::Nk3PqForwardSecure,
        other => panic!("unsupported suite {other}"),
    };

    PerfFixture {
        suite,
        kem_id: parsed.kem_id,
        sig_id: parsed.sig_id,
        client_capabilities: decode_hex(&parsed.inputs.client_capabilities_hex)
            .expect("client capabilities hex"),
        relay_capabilities: decode_hex(&parsed.inputs.relay_capabilities_hex)
            .expect("relay capabilities hex"),
        client_static_sk: decode_array(&parsed.inputs.client_static_sk_hex),
        relay_static_sk: decode_array(&parsed.inputs.relay_static_sk_hex),
        client_nonce: decode_array(&parsed.inputs.client_nonce_hex),
        relay_nonce: decode_array(&parsed.inputs.relay_nonce_hex),
        descriptor_commit: decode_array(&parsed.inputs.descriptor_commit_hex),
        resume_hash: parsed
            .inputs
            .resume_hash_hex
            .as_ref()
            .map(|hex| decode_hex(hex).expect("resume hash hex")),
    }
}

fn decode_array<const N: usize>(hex: &str) -> [u8; N] {
    let bytes = decode_hex(hex).expect("raw hex decode");
    if bytes.len() != N {
        panic!("expected {N} bytes, got {}", bytes.len());
    }
    let mut array = [0u8; N];
    array.copy_from_slice(&bytes);
    array
}

fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .expect("crate nested under tools")
        .parent()
        .expect("workspace root should exist")
        .to_path_buf()
}
