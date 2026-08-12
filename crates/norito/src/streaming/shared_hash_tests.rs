// Included inside the streaming tests module so later tests can reuse these helpers.

fn demo_hash(seed: u8) -> Hash {
    let mut bytes = [0u8; 32];
    bytes.fill(seed);
    bytes
}

fn demo_signature(seed: u8) -> Signature {
    let mut bytes = [0u8; 64];
    bytes.fill(seed);
    bytes
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        out.push(char::from(LUT[(byte >> 4) as usize]));
        out.push(char::from(LUT[(byte & 0x0f) as usize]));
    }
    out
}

#[test]
fn shared_blake3_adapter_matches_official_vectors() {
    assert_eq!(
        hex_encode(blake3_hash(b"")),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
    assert_eq!(
        hex_encode(blake3_hash(b"abc")),
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
    );

    let message = b"incremental content-addressed artifact verification";
    let mut incremental = Blake3Hasher::new();
    for chunk in message.chunks(3) {
        incremental.update(chunk);
    }
    assert_eq!(incremental.finalize(), blake3_hash(message));

    let mut empty = Blake3Hasher::default();
    empty.update(&[]);
    assert_eq!(empty.finalize(), blake3_hash(b""));
}
