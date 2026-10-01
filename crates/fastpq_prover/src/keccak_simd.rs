//! Two independent Keccak-f[1600] states in NEON/SSE2 lanes.
//!
//! Feature-gated intrinsics are isolated from the unsafe-free primitive crate.
//! Runtime feature detection precedes every accelerated call. The entire interleaved
//! state, preserved inactive lanes and theta/rho/pi scratch have clearing owners.
#![allow(unsafe_code)]
use super::{Job, RATE};
use core::ops::{Deref, DerefMut};
use zeroize::Zeroize;
struct Clearing<const N: usize>([[u64; 2]; N]);
impl<const N: usize> Clearing<N> {
    fn new(words: [[u64; 2]; N]) -> Self {
        Self(words)
    }
}
impl<const N: usize> Deref for Clearing<N> {
    type Target = [[u64; 2]; N];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<const N: usize> DerefMut for Clearing<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<const N: usize> Drop for Clearing<N> {
    fn drop(&mut self) {
        self.0.zeroize();
        #[cfg(test)]
        OBSERVATION.with(|cell| {
            if let Some((clean, bad)) = cell.get() {
                let cleared = self.0.iter().flatten().filter(|&&v| v == 0).count();
                cell.set(Some((clean + cleared, bad + 2 * N - cleared)));
            }
        });
    }
}
#[cfg(target_arch = "aarch64")]
pub(super) fn available() -> bool {
    std::arch::is_aarch64_feature_detected!("neon")
}
#[cfg(target_arch = "x86_64")]
pub(super) fn available() -> bool {
    std::arch::is_x86_feature_detected!("sse2")
}
const RC: [u64; 24] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808a,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808b,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008a,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000a,
    0x0000_0000_8000_808b,
    0x8000_0000_0000_008b,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800a,
    0x8000_0000_8000_000a,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];
const RHO: [u32; 25] = [
    0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14,
];
/// Both output arrays are completed public commitments; private arrays remain guarded.
pub(super) fn hash_pair(left: Job<'_>, right: Job<'_>) -> [[u8; 32]; 2] {
    let mut state = Clearing::new([[0u64; 2]; 25]);
    let jobs = [left, right];
    let mut positions = [0; 2];
    for (lane, job) in jobs.iter().enumerate() {
        job.prefix().with_absorbed_state_v1(|words, position| {
            positions[lane] = position;
            for (cell, &word) in state.iter_mut().zip(words) {
                cell[lane] = word;
            }
        });
    }
    let mut offsets = [0; 2];
    while offsets[0] < left.body().len() || offsets[1] < right.body().len() {
        let mut full = [false; 2];
        for lane in 0..2 {
            let count = (RATE - positions[lane]).min(jobs[lane].body().len() - offsets[lane]);
            for &byte in &jobs[lane].body()[offsets[lane]..offsets[lane] + count] {
                state[positions[lane] / 8][lane] ^= u64::from(byte) << ((positions[lane] % 8) * 8);
                positions[lane] += 1;
            }
            offsets[lane] += count;
            full[lane] = positions[lane] == RATE;
        }
        if full[0] || full[1] {
            // Heterogeneous lengths/positions are public. Preserve an inactive
            // lane under a clearing owner; do not accidentally permute it.
            let saved = Clearing::new(*state);
            permute(&mut state);
            for lane in 0..2 {
                if full[lane] {
                    positions[lane] = 0;
                } else {
                    for (cell, old) in state.iter_mut().zip(saved.iter()) {
                        cell[lane] = old[lane];
                    }
                }
            }
        }
    }
    for lane in 0..2 {
        state[positions[lane] / 8][lane] ^= 0x06u64 << ((positions[lane] % 8) * 8);
        state[16][lane] ^= 0x80u64 << 56;
    }
    permute(&mut state);
    let mut output = [[0; 32]; 2];
    for lane in 0..2 {
        for byte in 0..32 {
            output[lane][byte] = (state[byte / 8][lane] >> ((byte % 8) * 8)) as u8;
        }
    }
    output
}
fn permute(state: &mut [[u64; 2]; 25]) {
    // SAFETY: the sole production caller checks available() before hash_pair.
    // Loads/stores address full [u64;2] cells and permit their alignment.
    unsafe {
        vector_permute(state);
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn vector_permute(state: &mut [[u64; 2]; 25]) {
    use core::arch::aarch64::*;
    type V = uint64x2_t;
    // The closures stay within this target-feature function and never escape.
    unsafe {
        let load = |p: &[u64; 2]| -> V { vld1q_u64(p.as_ptr()) };
        let store = |p: &mut [u64; 2], v: V| vst1q_u64(p.as_mut_ptr(), v);
        let xor = |a: V, b: V| -> V { veorq_u64(a, b) };
        let andnot = |a: V, b: V| -> V {
            vandq_u64(
                vreinterpretq_u64_u32(vmvnq_u32(vreinterpretq_u32_u64(a))),
                b,
            )
        };
        let rotate = |a: V, n: u32| -> V {
            vorrq_u64(
                vshlq_u64(a, vdupq_n_s64(i64::from(n))),
                vshlq_u64(a, vdupq_n_s64(i64::from(n) - 64)),
            )
        };
        let duplicate = |v: u64| -> V { vdupq_n_u64(v) };
        let mut c = Clearing::new([[0u64; 2]; 5]);
        let mut d = Clearing::new([[0u64; 2]; 5]);
        let mut b = Clearing::new([[0u64; 2]; 25]);
        for rc in RC {
            for x in 0..5 {
                store(
                    &mut c[x],
                    xor(
                        xor(
                            xor(
                                xor(load(&state[x]), load(&state[x + 5])),
                                load(&state[x + 10]),
                            ),
                            load(&state[x + 15]),
                        ),
                        load(&state[x + 20]),
                    ),
                );
            }
            for x in 0..5 {
                store(
                    &mut d[x],
                    xor(load(&c[(x + 4) % 5]), rotate(load(&c[(x + 1) % 5]), 1)),
                );
                for y in 0..5 {
                    let a = xor(load(&state[x + 5 * y]), load(&d[x]));
                    store(&mut state[x + 5 * y], a);
                }
            }
            for x in 0..5 {
                for y in 0..5 {
                    store(
                        &mut b[y + 5 * ((2 * x + 3 * y) % 5)],
                        rotate(load(&state[x + 5 * y]), RHO[x + 5 * y]),
                    );
                }
            }
            for x in 0..5 {
                for y in 0..5 {
                    store(
                        &mut state[x + 5 * y],
                        xor(
                            load(&b[x + 5 * y]),
                            andnot(load(&b[(x + 1) % 5 + 5 * y]), load(&b[(x + 2) % 5 + 5 * y])),
                        ),
                    );
                }
            }
            let a = xor(load(&state[0]), duplicate(rc));
            store(&mut state[0], a);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn vector_permute(state: &mut [[u64; 2]; 25]) {
    use core::arch::x86_64::*;
    type V = __m128i;
    // The closures stay within this target-feature function and never escape.
    unsafe {
        let load = |p: &[u64; 2]| -> V { _mm_loadu_si128(p.as_ptr().cast()) };
        let store = |p: &mut [u64; 2], v: V| _mm_storeu_si128(p.as_mut_ptr().cast(), v);
        let xor = |a: V, b: V| -> V { _mm_xor_si128(a, b) };
        let andnot = |a: V, b: V| -> V { _mm_andnot_si128(a, b) };
        let rotate = |a: V, n: u32| -> V {
            _mm_or_si128(
                _mm_sll_epi64(a, _mm_cvtsi64_si128(i64::from(n))),
                _mm_srl_epi64(a, _mm_cvtsi64_si128(64 - i64::from(n))),
            )
        };
        let duplicate = |v: u64| -> V { _mm_set1_epi64x(v as i64) };
        let mut c = Clearing::new([[0u64; 2]; 5]);
        let mut d = Clearing::new([[0u64; 2]; 5]);
        let mut b = Clearing::new([[0u64; 2]; 25]);
        for rc in RC {
            for x in 0..5 {
                store(
                    &mut c[x],
                    xor(
                        xor(
                            xor(
                                xor(load(&state[x]), load(&state[x + 5])),
                                load(&state[x + 10]),
                            ),
                            load(&state[x + 15]),
                        ),
                        load(&state[x + 20]),
                    ),
                );
            }
            for x in 0..5 {
                store(
                    &mut d[x],
                    xor(load(&c[(x + 4) % 5]), rotate(load(&c[(x + 1) % 5]), 1)),
                );
                for y in 0..5 {
                    let a = xor(load(&state[x + 5 * y]), load(&d[x]));
                    store(&mut state[x + 5 * y], a);
                }
            }
            for x in 0..5 {
                for y in 0..5 {
                    store(
                        &mut b[y + 5 * ((2 * x + 3 * y) % 5)],
                        rotate(load(&state[x + 5 * y]), RHO[x + 5 * y]),
                    );
                }
            }
            for x in 0..5 {
                for y in 0..5 {
                    store(
                        &mut state[x + 5 * y],
                        xor(
                            load(&b[x + 5 * y]),
                            andnot(load(&b[(x + 1) % 5 + 5 * y]), load(&b[(x + 2) % 5 + 5 * y])),
                        ),
                    );
                }
            }
            let a = xor(load(&state[0]), duplicate(rc));
            store(&mut state[0], a);
        }
    }
}

#[cfg(test)]
std::thread_local! {static OBSERVATION:std::cell::Cell<Option<(usize,usize)>>=const {std::cell::Cell::new(None)};}
#[cfg(test)]
mod tests {
    use super::*;
    use fastpq_isi::keccak256::Sha3_256V1;
    struct Observation;
    impl Drop for Observation {
        fn drop(&mut self) {
            OBSERVATION.with(|c| c.set(None));
        }
    }
    #[test]
    fn actual_simd_state_saved_lanes_and_scratch_clear_on_return_and_unwind() {
        if !available() {
            return;
        }
        OBSERVATION.with(|c| assert_eq!(c.replace(Some((0, 0))), None));
        let _guard = Observation;
        let mut prefix = Sha3_256V1::new();
        prefix.update(&[0xA7; 135]);
        let jobs = [
            Job::new(&prefix, &[0x91; 273]),
            Job::new(&prefix, &[0x32; 7]),
        ];
        let result = hash_pair(jobs[0], jobs[1]);
        for i in 0..2 {
            assert_eq!(result[i], jobs[i].scalar().into_bytes());
        }
        let before = OBSERVATION.with(|c| c.get().unwrap());
        assert!(before.0 >= 50);
        assert_eq!(before.1, 0);
        assert!(
            std::panic::catch_unwind(|| {
                let _private = Clearing::new([[0xA7; 2]; 25]);
                panic!("injected private scratch unwind")
            })
            .is_err()
        );
        assert_eq!(OBSERVATION.with(|c| c.get().unwrap()), (before.0 + 50, 0));
    }
}
