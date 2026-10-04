//! AArch64 SHA256H/H2 rounds and the four-word SHA256SU0/SU1 schedule.

/// The caller must establish the `sha2` CPU feature before entry.
#[target_feature(enable = "sha2")]
pub(super) unsafe fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    use core::arch::aarch64::*;
    // Arm ACLE SHA256 operands: H(abcd, efgh, wk), H2(efgh, original_abcd, wk);
    // SU0(w0..3, w4..7), then SU1(partial, w8..11, w12..15).
    // https://arm-software.github.io/acle/neon_intrinsics/advsimd.html#sha256
    unsafe {
        let original_abcd = vld1q_u32(state.as_ptr());
        let original_efgh = vld1q_u32(state.as_ptr().add(4));
        let mut abcd = original_abcd;
        let mut efgh = original_efgh;
        let mut schedule: [uint32x4_t; 4] = std::array::from_fn(|index| {
            vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(block.as_ptr().add(index * 16))))
        });
        for round in 0..16 {
            let index = round % 4;
            let wk = vaddq_u32(
                schedule[index],
                vld1q_u32(crate::sha256_ref::SHA256_K.as_ptr().add(round * 4)),
            );
            let prior_abcd = abcd;
            abcd = vsha256hq_u32(abcd, efgh, wk);
            efgh = vsha256h2q_u32(efgh, prior_abcd, wk);
            if round < 12 {
                let partial = vsha256su0q_u32(schedule[index], schedule[(index + 1) % 4]);
                schedule[index] = vsha256su1q_u32(
                    partial,
                    schedule[(index + 2) % 4],
                    schedule[(index + 3) % 4],
                );
            }
        }
        vst1q_u32(state.as_mut_ptr(), vaddq_u32(original_abcd, abcd));
        vst1q_u32(state.as_mut_ptr().add(4), vaddq_u32(original_efgh, efgh));
    }
}
