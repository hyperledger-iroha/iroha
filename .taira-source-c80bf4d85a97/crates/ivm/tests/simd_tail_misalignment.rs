//! SIMD tail/misalignment parity tests
//!
//! These tests validate that chunked SIMD helpers (processing 4 lanes at a time)
//! produce identical results to scalar slice implementations when inputs are
//! arbitrarily offset (misaligned) and when lengths are not multiples of the
//! chunk size. This covers both tails and misaligned base pointers.
//!
//! The tests are deterministic and do not require any specific hardware
//! features. On platforms without SIMD, `ivm::vector` falls back to scalar
//! intrinsics, so parity still holds.

#![allow(clippy::needless_range_loop)]

use ivm::{vadd32, vadd64, vand, vor, vrot32, vxor};

// Simple deterministic PRNG (xoshiro-like) to avoid external deps.
#[derive(Clone)]
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u32(&mut self) -> u32 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545F4914F6CDD1D) >> 32) as u32
    }
    fn next_u8(&mut self) -> u8 {
        (self.next_u32() & 0xff) as u8
    }
}

fn scalar_add32(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for i in 0..a.len() {
        out[i] = a[i].wrapping_add(b[i]);
    }
}

fn simd_chunked_add32(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    let mut i = 0;
    while i + 4 <= a.len() {
        let va = [a[i], a[i + 1], a[i + 2], a[i + 3]];
        let vb = [b[i], b[i + 1], b[i + 2], b[i + 3]];
        let r = vadd32(va, vb);
        out[i..i + 4].copy_from_slice(&r);
        i += 4;
    }
    while i < a.len() {
        out[i] = a[i].wrapping_add(b[i]);
        i += 1;
    }
}

fn scalar_bit<F: Fn(u32, u32) -> u32>(a: &[u32], b: &[u32], out: &mut [u32], f: F) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for i in 0..a.len() {
        out[i] = f(a[i], b[i]);
    }
}

fn simd_chunked_bit(
    a: &[u32],
    b: &[u32],
    out: &mut [u32],
    simd: fn([u32; 4], [u32; 4]) -> [u32; 4],
    scalar: fn(u32, u32) -> u32,
) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    let mut i = 0;
    while i + 4 <= a.len() {
        let va = [a[i], a[i + 1], a[i + 2], a[i + 3]];
        let vb = [b[i], b[i + 1], b[i + 2], b[i + 3]];
        let r = simd(va, vb);
        out[i..i + 4].copy_from_slice(&r);
        i += 4;
    }
    while i < a.len() {
        out[i] = scalar(a[i], b[i]);
        i += 1;
    }
}

fn scalar_rot32(a: &[u32], k: u32, out: &mut [u32]) {
    assert_eq!(a.len(), out.len());
    for i in 0..a.len() {
        out[i] = a[i].rotate_left(k);
    }
}

fn simd_chunked_rot32(a: &[u32], k: u32, out: &mut [u32]) {
    assert_eq!(a.len(), out.len());
    let mut i = 0;
    while i + 4 <= a.len() {
        let va = [a[i], a[i + 1], a[i + 2], a[i + 3]];
        let r = vrot32(va, k);
        out[i..i + 4].copy_from_slice(&r);
        i += 4;
    }
    while i < a.len() {
        out[i] = a[i].rotate_left(k);
        i += 1;
    }
}

fn scalar_add64_pairs(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    assert!(a.len().is_multiple_of(2));
    for i in (0..a.len()).step_by(2) {
        let x = (a[i] as u64) | ((a[i + 1] as u64) << 32);
        let y = (b[i] as u64) | ((b[i + 1] as u64) << 32);
        let r = x.wrapping_add(y);
        out[i] = (r & 0xffff_ffff) as u32;
        out[i + 1] = (r >> 32) as u32;
    }
}

fn simd_chunked_add64_pairs(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    assert!(a.len().is_multiple_of(2));
    let mut i = 0;
    while i + 4 <= a.len() {
        let va = [a[i], a[i + 1], a[i + 2], a[i + 3]];
        let vb = [b[i], b[i + 1], b[i + 2], b[i + 3]];
        let r = vadd64(va, vb);
        out[i..i + 4].copy_from_slice(&r);
        i += 4;
    }
    while i < a.len() {
        // Process last pair (cannot be a single leftover if len is even)
        let x = (a[i] as u64) | ((a[i + 1] as u64) << 32);
        let y = (b[i] as u64) | ((b[i + 1] as u64) << 32);
        let r = x.wrapping_add(y);
        out[i] = (r & 0xffff_ffff) as u32;
        out[i + 1] = (r >> 32) as u32;
        i += 2;
    }
}

#[test]
fn simd_tail_and_misalignment_add32_parity() {
    let mut rng = Rng::new(0xDEADBEEFCAFEBABE);
    // Multiple seeds/lengths/offsets
    for _case in 0..128 {
        let len = (rng.next_u32() as usize % 257) + 1; // 1..=257 to cover small/medium
        let off_a = (rng.next_u8() % 4) as usize;
        let off_b = (rng.next_u8() % 4) as usize;

        let mut buf_a = vec![0u32; len + 8];
        let mut buf_b = vec![0u32; len + 8];
        for v in &mut buf_a {
            *v = rng.next_u32();
        }
        for v in &mut buf_b {
            *v = rng.next_u32();
        }
        let a = &buf_a[off_a..off_a + len];
        let b = &buf_b[off_b..off_b + len];

        let mut out_scalar = vec![0u32; len];
        let mut out_simd = vec![0u32; len];
        scalar_add32(a, b, &mut out_scalar);
        simd_chunked_add32(a, b, &mut out_simd);
        assert_eq!(
            out_scalar, out_simd,
            "len={len}, off_a={off_a}, off_b={off_b}"
        );
    }
}

#[test]
fn simd_tail_and_misalignment_bitwise_parity() {
    let mut rng = Rng::new(0xABCDEF0123456789);
    for _case in 0..128 {
        let len = (rng.next_u32() as usize % 257) + 1;
        let off_a = (rng.next_u8() % 4) as usize;
        let off_b = (rng.next_u8() % 4) as usize;

        let mut buf_a = vec![0u32; len + 8];
        let mut buf_b = vec![0u32; len + 8];
        for v in &mut buf_a {
            *v = rng.next_u32();
        }
        for v in &mut buf_b {
            *v = rng.next_u32();
        }
        let a = &buf_a[off_a..off_a + len];
        let b = &buf_b[off_b..off_b + len];

        // AND
        let mut s = vec![0u32; len];
        let mut d = vec![0u32; len];
        scalar_bit(a, b, &mut s, |x, y| x & y);
        simd_chunked_bit(a, b, &mut d, vand, |x, y| x & y);
        assert_eq!(s, d, "AND len={len}, off_a={off_a}, off_b={off_b}");

        // XOR
        scalar_bit(a, b, &mut s, |x, y| x ^ y);
        simd_chunked_bit(a, b, &mut d, vxor, |x, y| x ^ y);
        assert_eq!(s, d, "XOR len={len}, off_a={off_a}, off_b={off_b}");

        // OR
        scalar_bit(a, b, &mut s, |x, y| x | y);
        simd_chunked_bit(a, b, &mut d, vor, |x, y| x | y);
        assert_eq!(s, d, "OR len={len}, off_a={off_a}, off_b={off_b}");
    }
}

#[test]
fn simd_tail_and_misalignment_rot32_parity() {
    let mut rng = Rng::new(0x0123_4567_89AB_CDEF);
    for _case in 0..128 {
        let len = (rng.next_u32() as usize % 257) + 1;
        let off = (rng.next_u8() % 4) as usize;
        let mut buf = vec![0u32; len + 8];
        for v in &mut buf {
            *v = rng.next_u32();
        }
        let a = &buf[off..off + len];
        let k = (rng.next_u8() % 32) as u32;

        let mut s = vec![0u32; len];
        let mut d = vec![0u32; len];
        scalar_rot32(a, k, &mut s);
        simd_chunked_rot32(a, k, &mut d);
        assert_eq!(s, d, "ROT len={len}, off={off}, k={k}");
    }
}

#[test]
fn simd_tail_and_misalignment_add64_parity_even_lengths() {
    let mut rng = Rng::new(0x1337_C0DE_F00D_BAAD);
    for _case in 0..128 {
        // Ensure even length to match 64-bit pair semantics
        let len = (((rng.next_u32() as usize % 256) + 2) / 2) * 2; // even in [2, 256]
        let off_a = (rng.next_u8() % 2) as usize * 2; // keep base 8-byte aligned vs unaligned in 8-byte terms
        let off_b = (rng.next_u8() % 2) as usize * 2;

        let mut buf_a = vec![0u32; len + 8];
        let mut buf_b = vec![0u32; len + 8];
        for v in &mut buf_a {
            *v = rng.next_u32();
        }
        for v in &mut buf_b {
            *v = rng.next_u32();
        }
        let a = &buf_a[off_a..off_a + len];
        let b = &buf_b[off_b..off_b + len];

        let mut s = vec![0u32; len];
        let mut d = vec![0u32; len];
        scalar_add64_pairs(a, b, &mut s);
        simd_chunked_add64_pairs(a, b, &mut d);
        assert_eq!(s, d, "ADD64 len={len}, off_a={off_a}, off_b={off_b}");
    }
}
