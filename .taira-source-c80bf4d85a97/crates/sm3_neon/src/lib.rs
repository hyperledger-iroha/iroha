#![allow(clippy::missing_const_for_fn)]

//! NEON helpers for the SM3 hash function.
//!
//! The routine below implements the SM3 compression function and padding while
//! keeping the API surface aligned with the `sm4-neon` helper. The current
//! implementation still mirrors the scalar RustCrypto path but executes inside
//! an AArch64 NEON context so additional vectorisation can be layered in
//! step-by-step.

#[cfg(target_arch = "aarch64")]
mod neon {
    use core::arch::aarch64::*;
    use std::vec::Vec;

    #[inline]
    pub fn is_supported() -> bool {
        cfg!(feature = "force-neon") || std::arch::is_aarch64_feature_detected!("neon")
    }

    /// Compute an SM3 digest using the NEON path. Returns `None` when NEON is
    /// unavailable so callers can fall back to the scalar routine.
    pub fn digest(message: &[u8]) -> Option<[u8; 32]> {
        if !is_supported() {
            return None;
        }
        Some(unsafe { digest_neon(message) })
    }

    #[target_feature(enable = "neon")]
    fn digest_neon(message: &[u8]) -> [u8; 32] {
        const IV: [u32; 8] = [
            0x7380_166F,
            0x4914_B2B9,
            0x1724_42D7,
            0xDA8A_0600,
            0xA96F_30BC,
            0x1631_38AA,
            0xE38D_EE4D,
            0xB0FB_0E4E,
        ];

        let mut state = IV;
        let buffer = allocate_padded(message);
        for chunk in buffer.chunks_exact(64) {
            compress_block(&mut state, chunk);
        }
        let mut out = [0u8; 32];
        for (slot, value) in out.chunks_exact_mut(4).zip(state.iter()) {
            slot.copy_from_slice(&value.to_be_bytes());
        }
        out
    }

    #[inline]
    #[target_feature(enable = "neon")]
    fn compress_block(state: &mut [u32; 8], block: &[u8]) {
        debug_assert_eq!(block.len(), 64);

        let mut w = [0u32; 68];
        let mut w_prime = [0u32; 64];

        for (i, chunk) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes(chunk.try_into().expect("SM3 chunk"));
        }

        for j in 16..68 {
            let x = w[j - 16] ^ w[j - 9] ^ rotl32(w[j - 3], 15);
            w[j] = p1(x) ^ rotl32(w[j - 13], 7) ^ w[j - 6];
        }
        compute_w_prime(&w, &mut w_prime);

        let mut t_rot = [0u32; 64];
        for (idx, slot) in t_rot.iter_mut().enumerate() {
            let t: u32 = if idx < 16 { 0x79CC_4519 } else { 0x7A87_9D8A };
            *slot = t.rotate_left(idx as u32);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        let mut j = 0usize;
        while j < 64 {
            let w_lane = unsafe {
                core::mem::transmute::<uint32x4_t, [u32; 4]>(vld1q_u32(w.as_ptr().add(j)))
            };
            let w_prime_lane = unsafe {
                core::mem::transmute::<uint32x4_t, [u32; 4]>(vld1q_u32(w_prime.as_ptr().add(j)))
            };
            let t_lane = unsafe {
                core::mem::transmute::<uint32x4_t, [u32; 4]>(vld1q_u32(t_rot.as_ptr().add(j)))
            };

            let lane_count = (64 - j).min(4);
            for lane in 0..lane_count {
                let index = j + lane;
                let a_rotl12 = rotl32(a, 12);
                let ss1 = rotl32(a_rotl12.wrapping_add(e).wrapping_add(t_lane[lane]), 7);
                let ss2 = ss1 ^ a_rotl12;
                let tt1 = ff(index, a, b, c)
                    .wrapping_add(d)
                    .wrapping_add(ss2)
                    .wrapping_add(w_prime_lane[lane]);
                let tt2 = gg(index, e, f, g)
                    .wrapping_add(h)
                    .wrapping_add(ss1)
                    .wrapping_add(w_lane[lane]);
                d = c;
                c = rotl32(b, 9);
                b = a;
                a = tt1;
                h = g;
                g = rotl32(f, 19);
                f = e;
                e = p0(tt2);
            }

            j += 4;
        }

        state[0] ^= a;
        state[1] ^= b;
        state[2] ^= c;
        state[3] ^= d;
        state[4] ^= e;
        state[5] ^= f;
        state[6] ^= g;
        state[7] ^= h;
    }

    #[inline]
    #[target_feature(enable = "neon")]
    fn p0(x: u32) -> u32 {
        x ^ rotl32(x, 9) ^ rotl32(x, 17)
    }

    #[inline]
    #[target_feature(enable = "neon")]
    fn p1(x: u32) -> u32 {
        x ^ rotl32(x, 15) ^ rotl32(x, 23)
    }

    #[inline]
    #[target_feature(enable = "neon")]
    fn ff(j: usize, x: u32, y: u32, z: u32) -> u32 {
        if j < 16 {
            x ^ y ^ z
        } else {
            (x & y) | (x & z) | (y & z)
        }
    }

    #[inline]
    #[target_feature(enable = "neon")]
    fn gg(j: usize, x: u32, y: u32, z: u32) -> u32 {
        if j < 16 {
            x ^ y ^ z
        } else {
            (x & y) | ((!x) & z)
        }
    }

    #[inline]
    #[target_feature(enable = "neon")]
    fn rotl32(value: u32, n: u32) -> u32 {
        value.rotate_left(n)
    }

    #[inline]
    fn allocate_padded(message: &[u8]) -> Vec<u8> {
        let len = message.len();
        let len_bits = (len as u64) * 8;
        let padded_blocks = (len + 9).div_ceil(64);
        let total_len = padded_blocks * 64;
        let mut buffer = vec![0u8; total_len];
        buffer[..len].copy_from_slice(message);
        buffer[len] = 0x80;
        let tail_offset = total_len
            .checked_sub(8)
            .expect("SM3 padded length always >= 8");
        buffer[tail_offset..].copy_from_slice(&len_bits.to_be_bytes());
        buffer
    }

    #[inline]
    #[target_feature(enable = "neon")]
    fn compute_w_prime(w: &[u32; 68], w_prime: &mut [u32; 64]) {
        // Compute W' schedule using 128-bit NEON XOR so four words are processed per
        // iteration. The scalar path remains available as a fallback in the SM3
        // reference implementation; this helper strictly accelerates the NEON build.
        unsafe {
            let mut idx = 0usize;
            while idx < 64 {
                let base = vld1q_u32(w.as_ptr().add(idx));
                let ahead = vld1q_u32(w.as_ptr().add(idx + 4));
                let combined = veorq_u32(base, ahead);
                vst1q_u32(w_prime.as_mut_ptr().add(idx), combined);
                idx += 4;
            }
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub use neon::{digest, is_supported};

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub fn is_supported() -> bool {
    false
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub fn digest(_: &[u8]) -> Option<[u8; 32]> {
    None
}

#[cfg(all(test, target_arch = "aarch64"))]
mod tests {
    use hex_literal::hex;
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use sm3::{Digest, Sm3};

    use super::*;

    #[test]
    fn digest_matches_scalar() {
        if !is_supported() {
            eprintln!("skipping SM3 NEON test: NEON not available");
            return;
        }

        let vectors = [
            b"".as_slice(),
            b"abc".as_slice(),
            b"abcdefghijklmnopqrstuvwxyz0123456789".as_slice(),
            &[0u8; 128],
        ];

        for message in vectors {
            assert_eq!(
                scalar_digest(message),
                digest(message).expect("NEON path"),
                "SM3 NEON mismatch for vector {message:?}"
            );
        }

        let mut rng = StdRng::seed_from_u64(0xDEADBEEFCAFEBABE);
        for size in [
            1usize, 3, 7, 16, 31, 63, 64, 65, 127, 255, 256, 511, 512, 513,
        ] {
            let mut msg = vec![0u8; size];
            rng.fill(&mut msg[..]);
            assert_eq!(
                scalar_digest(&msg),
                digest(&msg).expect("NEON path"),
                "SM3 NEON mismatch for random len {size}"
            );
        }
    }

    #[test]
    fn digest_matches_golden_vectors() {
        if !is_supported() {
            eprintln!("skipping SM3 NEON golden test: NEON not available");
            return;
        }

        const LONG_BLOCK: &[u8] =
            b"abcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcd";
        const GOLDEN: [(&[u8], [u8; 32]); 3] = [
            (
                b"",
                hex!("1AB21D8355CFA17F8E61194831E81A8F22BEC8C728FEFB747ED035EB5082AA2B"),
            ),
            (
                b"abc",
                hex!("66C7F0F462EEEDD9D1F2D46BDC10E4E24167C4875CF2F7A2297DA02B8F4BA8E0"),
            ),
            (
                LONG_BLOCK,
                hex!("DEBE9FF92275B8A138604889C18E5A4D6FDB70E5387E5765293DCBA39C0C5732"),
            ),
        ];

        for (message, expected) in GOLDEN {
            let digest = super::digest(message).expect("NEON path");
            assert_eq!(
                digest, expected,
                "SM3 NEON digest mismatch for message {message:?}"
            );
        }
    }

    fn scalar_digest(message: &[u8]) -> [u8; 32] {
        let mut hasher = Sm3::new();
        hasher.update(message);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hasher.finalize());
        out
    }
}
