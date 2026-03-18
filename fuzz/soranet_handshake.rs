#![no_main]

use arbitrary::Arbitrary;
use iroha_crypto::{
    Algorithm, KeyPair,
    soranet::handshake::{
        DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
        HandshakeSuite, RuntimeParams, SimulationParams, build_client_hello,
        client_handle_relay_hello, relay_finalize_handshake, simulate_handshake,
        simulation_report_json,
    },
};
use libfuzzer_sys::fuzz_target;
use rand::{SeedableRng as _, rngs::ChaCha20Rng};

#[derive(Debug, Arbitrary)]
struct ByteMutation {
    index: u8,
    value: u8,
}

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    client_suite_bytes: [u8; 4],
    relay_suite_bytes: [u8; 4],
    client_required: bool,
    relay_required: bool,
    client_mutations: Vec<ByteMutation>,
    relay_mutations: Vec<ByteMutation>,
    resume_hash: Option<[u8; 32]>,
    descriptor_commit: [u8; 32],
    client_nonce: [u8; 32],
    relay_nonce: [u8; 32],
    client_static_sk: [u8; 32],
    relay_static_sk: [u8; 32],
    kem_id: u8,
    sig_id: u8,
    client_seed: [u8; 32],
    relay_seed: [u8; 32],
    key_seed: [u8; 32],
}

fn build_suite_order(bytes: &[u8]) -> Vec<HandshakeSuite> {
    let mut suites = Vec::new();
    for &raw in bytes {
        let candidate = match raw % 2 {
            0 => HandshakeSuite::Nk2Hybrid,
            _ => HandshakeSuite::Nk3PqForwardSecure,
        };
        if !suites.contains(&candidate) {
            suites.push(candidate);
        }
    }
    if suites.is_empty() {
        suites.push(HandshakeSuite::Nk2Hybrid);
        suites.push(HandshakeSuite::Nk3PqForwardSecure);
    }
    suites
}

fn encode_suite_list(suites: &[HandshakeSuite], required: bool) -> Vec<u8> {
    let mut bytes = suites
        .iter()
        .map(|suite| u8::from(*suite))
        .collect::<Vec<_>>();
    if let Some(first) = bytes.first_mut() {
        if required {
            *first |= 0x80;
        } else {
            *first &= 0x7F;
        }
    }
    bytes
}

fn rewrite_suite_list(buf: &mut Vec<u8>, suites: &[HandshakeSuite], required: bool) {
    const SUITE_TLV: u16 = 0x0104;
    let mut offset = 0;
    while offset + 4 <= buf.len() {
        let ty = u16::from_be_bytes([buf[offset], buf[offset + 1]]);
        let len = u16::from_be_bytes([buf[offset + 2], buf[offset + 3]]) as usize;
        let start = offset + 4;
        let end = start + len;
        if ty == SUITE_TLV {
            let encoded = encode_suite_list(suites, required);
            let new_len = encoded.len() as u16;
            buf[offset + 2] = (new_len >> 8) as u8;
            buf[offset + 3] = (new_len & 0xFF) as u8;
            buf.splice(start..end, encoded.iter().copied());
            return;
        }
        offset = end;
    }
}

fn apply_mutations(buf: &mut [u8], mutations: &[ByteMutation]) {
    if buf.is_empty() {
        return;
    }
    let len = buf.len();
    for mutation in mutations.iter().take(16) {
        let idx = usize::from(mutation.index) % len;
        buf[idx] ^= mutation.value;
    }
}

fn seed_rng(seed_bytes: &[u8; 32]) -> ChaCha20Rng {
    ChaCha20Rng::from_seed(*seed_bytes)
}

fn seeded_keypair(seed: &[u8; 32]) -> KeyPair {
    KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519)
}

fn run_simulation(case: &FuzzInput) {
    let mut client_caps = DEFAULT_CLIENT_CAPABILITIES.to_vec();
    let mut relay_caps = DEFAULT_RELAY_CAPABILITIES.to_vec();

    let client_suites = build_suite_order(&case.client_suite_bytes);
    let relay_suites = build_suite_order(&case.relay_suite_bytes);
    rewrite_suite_list(&mut client_caps, &client_suites, case.client_required);
    rewrite_suite_list(&mut relay_caps, &relay_suites, case.relay_required);
    apply_mutations(&mut client_caps, &case.client_mutations);
    apply_mutations(&mut relay_caps, &case.relay_mutations);

    let resume_vec = case.resume_hash.map(<[u8; 32]>::to_vec);
    let resume_slice = resume_vec.as_deref();

    let descriptor = if case.descriptor_commit == [0u8; 32] {
        DEFAULT_DESCRIPTOR_COMMIT.to_vec()
    } else {
        case.descriptor_commit.to_vec()
    };

    let kem_id = case.kem_id % 3;
    let sig_id = if case.sig_id == 0 { 1 } else { case.sig_id };

    let params = SimulationParams {
        client_capabilities: &client_caps,
        relay_capabilities: &relay_caps,
        client_static_sk: case.client_static_sk.as_slice(),
        relay_static_sk: case.relay_static_sk.as_slice(),
        resume_hash: resume_slice,
        descriptor_commit: &descriptor,
        client_nonce: case.client_nonce.as_slice(),
        relay_nonce: case.relay_nonce.as_slice(),
        kem_id,
        sig_id,
    };

    if let Ok(result) = simulate_handshake(&params) {
        // Exercise JSON rendering path; ignore serialization failures.
        let _ = simulation_report_json(&result, None::<&[u16]>);
    }
}

fn run_runtime_handshake(case: &FuzzInput) {
    let mut client_caps = DEFAULT_CLIENT_CAPABILITIES.to_vec();
    let mut relay_caps = DEFAULT_RELAY_CAPABILITIES.to_vec();
    let client_suites = build_suite_order(&case.client_suite_bytes);
    let relay_suites = build_suite_order(&case.relay_suite_bytes);
    rewrite_suite_list(&mut client_caps, &client_suites, case.client_required);
    rewrite_suite_list(&mut relay_caps, &relay_suites, case.relay_required);
    apply_mutations(&mut client_caps, &case.client_mutations);
    apply_mutations(&mut relay_caps, &case.relay_mutations);

    let resume_vec = case.resume_hash.map(<[u8; 32]>::to_vec);
    let resume_slice = resume_vec.as_deref();
    let descriptor = if case.descriptor_commit == [0u8; 32] {
        DEFAULT_DESCRIPTOR_COMMIT.as_slice()
    } else {
        case.descriptor_commit.as_slice()
    };

    let runtime = RuntimeParams {
        descriptor_commit,
        client_capabilities: &client_caps,
        relay_capabilities: &relay_caps,
        kem_id: case.kem_id % 3,
        sig_id: if case.sig_id == 0 { 1 } else { case.sig_id },
        resume_hash: resume_slice,
    };

    let mut rng_client = seed_rng(&case.client_seed);
    let mut rng_relay = seed_rng(&case.relay_seed);
    let client_keys = seeded_keypair(&case.key_seed);
    let relay_keys = seeded_keypair(&case.relay_seed);

    let Ok((client_hello, client_state)) = build_client_hello(&runtime, &mut rng_client) else {
        return;
    };
    let Ok((relay_message, relay_state)) = iroha_crypto::soranet::handshake::process_client_hello(
        &client_hello,
        &runtime,
        &relay_keys,
        &mut rng_relay,
    ) else {
        return;
    };
    let Ok((client_finish, _client_session)) = client_handle_relay_hello(
        client_state,
        &relay_message,
        &client_keys,
        &runtime,
        &mut rng_client,
    ) else {
        return;
    };

    let _ = relay_finalize_handshake(
        relay_state,
        client_finish.as_deref().unwrap_or(&[]),
        &relay_keys,
    );
}

fuzz_target!(|case: FuzzInput| {
    run_simulation(&case);
    run_runtime_handshake(&case);
});
