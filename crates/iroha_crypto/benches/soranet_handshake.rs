//! Criterion benchmarks for `SoraNet` handshake state-machine cycles.

use criterion::Criterion;
use iroha_crypto::{
    Algorithm, KeyPair,
    soranet::handshake::{
        DEFAULT_DESCRIPTOR_COMMIT, HandshakeSuite, RuntimeParams, build_client_hello,
        client_handle_relay_hello, relay_finalize_handshake,
    },
};
use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

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

fn run_handshake(preferred: HandshakeSuite) {
    let scenario = HandshakeScenario::new(preferred);
    let params = scenario.params();

    let mut rng_client = ChaCha20Rng::from_seed([0xA5; 32]);
    let mut rng_relay = ChaCha20Rng::from_seed([0x5A; 32]);
    let client_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let relay_keys = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);

    let (client_hello, client_state) =
        build_client_hello(&params, &mut rng_client).expect("client hello");
    let (relay_message, relay_state) = iroha_crypto::soranet::handshake::process_client_hello(
        &client_hello,
        &params,
        &relay_keys,
        &mut rng_relay,
    )
    .expect("relay processing");
    let (client_finish, _) = client_handle_relay_hello(
        client_state,
        &relay_message,
        &client_keys,
        &params,
        &mut rng_client,
    )
    .expect("client finish");

    let finish = client_finish.as_deref().unwrap_or(&[]);
    relay_finalize_handshake(relay_state, finish, &relay_keys).expect("relay finalize");
}

fn soranet_handshake_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("soranet_handshake_cycle");
    group.sample_size(60);
    for (label, suite) in [
        ("nk2_hybrid", HandshakeSuite::Nk2Hybrid),
        ("nk3_forward_secure", HandshakeSuite::Nk3PqForwardSecure),
    ] {
        group.bench_function(label, |b| b.iter(|| run_handshake(suite)));
    }
    group.finish();
}

/// Run the `SoraNet` handshake Criterion benchmarks.
fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    soranet_handshake_benchmarks(&mut criterion);
    criterion.final_summary();
}

fn decode_hex_vec(label: &str, hex_str: &str) -> Vec<u8> {
    hex::decode(hex_str).unwrap_or_else(|err| panic!("{label} hex decode failed: {err}"))
}

fn load_nk2_caps() -> (Vec<u8>, Vec<u8>) {
    let raw = include_str!("../../../tests/interop/soranet/interop/rust/snnet-interop-nk2-v1.json");
    let value: norito::json::Value =
        norito::json::from_str(raw).expect("parse nk2 interop fixture");
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
    let raw = include_str!("../../../tests/interop/soranet/interop/rust/snnet-interop-nk3-v1.json");
    let value: norito::json::Value =
        norito::json::from_str(raw).expect("parse nk3 interop fixture");
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
