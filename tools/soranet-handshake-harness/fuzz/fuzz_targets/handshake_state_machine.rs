#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use rand::{SeedableRng, rngs::StdRng};
use soranet_handshake_harness::{
    KeyPair, RuntimeParams, process_client_hello, relay_finalize_handshake,
};

#[derive(Arbitrary, Debug)]
struct HandshakeInput {
    client_hello: Vec<u8>,
    client_finish: Vec<u8>,
}

fuzz_target!(|input: HandshakeInput| {
    // Use deterministic RNG so reproducing failures yields identical transcripts.
    let mut rng = StdRng::seed_from_u64(0x5eeda11f_u64);
    let params = RuntimeParams::soranet_defaults();
    let key_pair = KeyPair::random();

    if let Ok((_, relay_state)) = process_client_hello(
        input.client_hello.as_slice(),
        &params,
        &key_pair,
        &mut rng,
    ) {
        // ClientFinish frames are no longer used; ensure extra bytes are handled safely.
        let _ = relay_finalize_handshake(relay_state, input.client_finish.as_slice(), &key_pair);
    }
});
