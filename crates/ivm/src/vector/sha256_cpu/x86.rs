//! SHA-NI rounds in ABEF/CDGH order with the four-word MSG1/MSG2 schedule.

/// The caller must establish the `sha` and `ssse3` CPU features before entry.
#[target_feature(enable = "sha,ssse3")]
pub(super) unsafe fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    use core::arch::x86_64::*;
    // Intel SHA256RNDS2 consumes CDGH and ABEF (high-to-low lanes), returning
    // the next ABEF; the previous ABEF becomes CDGH after two rounds.
    // https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sha-extensions.html
    unsafe {
        let saved_abef = _mm_set_epi32(
            state[0] as i32,
            state[1] as i32,
            state[4] as i32,
            state[5] as i32,
        );
        let saved_cdgh = _mm_set_epi32(
            state[2] as i32,
            state[3] as i32,
            state[6] as i32,
            state[7] as i32,
        );
        let mut abef = saved_abef;
        let mut cdgh = saved_cdgh;
        let big_endian = _mm_set_epi8(12, 13, 14, 15, 8, 9, 10, 11, 4, 5, 6, 7, 0, 1, 2, 3);
        let mut schedule: [__m128i; 4] = std::array::from_fn(|index| {
            _mm_shuffle_epi8(
                _mm_loadu_si128(block.as_ptr().add(index * 16).cast()),
                big_endian,
            )
        });
        for round in 0..16 {
            let index = round % 4;
            let wk = _mm_add_epi32(
                schedule[index],
                _mm_loadu_si128(crate::sha256_ref::SHA256_K.as_ptr().add(round * 4).cast()),
            );
            cdgh = _mm_sha256rnds2_epu32(cdgh, abef, wk);
            abef = _mm_sha256rnds2_epu32(abef, cdgh, _mm_shuffle_epi32(wk, 0x0e));
            if round < 12 {
                let partial = _mm_sha256msg1_epu32(schedule[index], schedule[(index + 1) % 4]);
                let middle =
                    _mm_alignr_epi8(schedule[(index + 3) % 4], schedule[(index + 2) % 4], 4);
                schedule[index] =
                    _mm_sha256msg2_epu32(_mm_add_epi32(partial, middle), schedule[(index + 3) % 4]);
            }
        }
        let mut abef_words = [0_u32; 4];
        let mut cdgh_words = [0_u32; 4];
        _mm_storeu_si128(
            abef_words.as_mut_ptr().cast(),
            _mm_add_epi32(abef, saved_abef),
        );
        _mm_storeu_si128(
            cdgh_words.as_mut_ptr().cast(),
            _mm_add_epi32(cdgh, saved_cdgh),
        );
        *state = [
            abef_words[3],
            abef_words[2],
            cdgh_words[3],
            cdgh_words[2],
            abef_words[1],
            abef_words[0],
            cdgh_words[1],
            cdgh_words[0],
        ];
    }
}
