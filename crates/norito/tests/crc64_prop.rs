use norito::{crc64_fallback, hardware_crc64};

#[test]
fn random_lengths_parity_small() {
    // Deterministic pseudo-random stream
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut buf = vec![0u8; 8192];
    for byte in &mut buf {
        x ^= x << 7;
        x ^= x >> 9;
        *byte = (x as u8) ^ 0xA5;
    }

    for len in [
        0usize, 1, 7, 8, 15, 16, 31, 32, 63, 64, 511, 512, 2047, 2048, 4095, 4096,
    ] {
        let data = &buf[..len];
        assert_eq!(hardware_crc64(data), crc64_fallback(data), "len={len}");
    }
}

#[test]
fn random_windows_parity() {
    // Sliding windows over a larger buffer
    let mut x: u64 = 0xD1B5_FC71_02F9_8743;
    let mut buf = vec![0u8; 32 * 1024];
    for b in &mut buf {
        x ^= x << 11;
        x ^= x >> 7;
        *b = (x as u8) ^ 0x3C;
    }

    let sizes = [33usize, 127, 256, 1024, 4096, 8191];
    for &win in &sizes {
        let mut i = 0;
        while i + win <= buf.len() {
            let data = &buf[i..i + win];
            assert_eq!(
                hardware_crc64(data),
                crc64_fallback(data),
                "win={win} at {i}",
            );
            i += 997; // prime-ish step to vary alignment
        }
    }
}
