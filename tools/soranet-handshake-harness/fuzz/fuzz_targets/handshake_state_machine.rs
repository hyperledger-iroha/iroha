#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use rand::{SeedableRng, rngs::StdRng};
use soranet_handshake_harness::{
    Algorithm, KeyPair, RelayAuthenticationSignerV1, RuntimeParams, build_client_hello,
    client_handle_relay_hello, process_client_hello,
};
use std::sync::Arc;
#[derive(Arbitrary, Debug)]
struct HandshakeInput {
    client_hello: Vec<u8>,
    relay_response: Vec<u8>,
}
fuzz_target!(|input: HandshakeInput| {
    // Keep parser-side RNG and identities deterministic so failures reproduce.
    let mut relay_rng = StdRng::seed_from_u64(0x5eeda11f_u64);
    let mut client_rng = StdRng::seed_from_u64(0xc11e17_u64);
    let params = RuntimeParams::soranet_defaults();
    let ed25519 = KeyPair::try_from_seed(vec![0x45; 32], Algorithm::Ed25519)
        .expect("derive fuzz Ed25519 relay identity");
    let mldsa65 = KeyPair::try_from_seed(vec![0xE0; 32], Algorithm::MlDsa)
        .expect("derive fuzz ML-DSA-65 relay identity");
    let relay_authentication =
        RelayAuthenticationSignerV1::try_new(Arc::new(ed25519), Arc::new(mldsa65), [0xB7; 32])
            .expect("construct fuzz relay authentication");
    let _ = process_client_hello(
        input.client_hello.as_slice(),
        &params,
        &relay_authentication,
        &mut relay_rng,
    );
    if let Ok((_client_hello, client_state)) = build_client_hello(&params, &mut client_rng) {
        let _ = client_handle_relay_hello(
            client_state,
            input.relay_response.as_slice(),
            &relay_authentication.verifier(),
            &params,
        );
    }
});
