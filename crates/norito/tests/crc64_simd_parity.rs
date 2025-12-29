#![cfg(feature = "simd-accel")]
//! Cross-validate hardware_crc64 against the portable fallback across sizes and alignments.

use norito::core::{crc64_fallback, hardware_crc64};
use rand::{Rng, SeedableRng, rngs::StdRng};

#[test]
fn crc64_simd_matches_fallback_small_sizes() {
    for n in 0..256usize {
        let mut buf = vec![0u8; n];
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = i as u8;
        }
        let a = hardware_crc64(&buf);
        let b = crc64_fallback(&buf);
        assert_eq!(a, b, "n={} simd vs fallback", n);
    }
}

#[test]
fn crc64_simd_matches_fallback_random_large() {
    let mut rng = StdRng::seed_from_u64(0xDEADBEEFCAFEBABE);
    let sizes = [1024usize, 4096, 16384, 131072, 104857];
    for &n in &sizes {
        let mut buf = vec![0u8; n];
        rng.fill(&mut buf[..]);
        // Aligned slice
        let a = hardware_crc64(&buf);
        let b = crc64_fallback(&buf);
        assert_eq!(a, b, "aligned n={}", n);
        // Misaligned prefix view when possible
        if n > 3 {
            let a = hardware_crc64(&buf[3..]);
            let b = crc64_fallback(&buf[3..]);
            assert_eq!(a, b, "misaligned n={} off=3", n);
        }
        if n > 1 {
            let a = hardware_crc64(&buf[1..]);
            let b = crc64_fallback(&buf[1..]);
            assert_eq!(a, b, "misaligned n={} off=1", n);
        }
    }
}
