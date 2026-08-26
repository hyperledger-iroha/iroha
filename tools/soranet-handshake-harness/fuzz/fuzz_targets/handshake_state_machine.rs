#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use rand::{SeedableRng, rngs::StdRng};
use soranet_handshake_harness::{KeyPair, RuntimeParams, process_client_hello};
#[derive(Arbitrary, Debug)]
struct HandshakeInput {
    client_hello: Vec<u8>,
}
fuzz_target!(|input: HandshakeInput| {
    // Use deterministic RNG so reproducing failures yields identical transcripts.
    let mut rng = StdRng::seed_from_u64(0x5eeda11f_u64);
    let params = RuntimeParams::soranet_defaults();
    let key_pair = KeyPair::random();
    let _ = process_client_hello(input.client_hello.as_slice(), &params, &key_pair, &mut rng);
});
