//! Audited temporary allocation demand for one exact-NFC comparison.
//!
//! The baked profile bounds recursive decomposition by four scalars per source
//! scalar. ICU4X 2.2.0 `Decomposition` owns one `SmallVec<[CharacterAndClass; 17]>`,
//! with a four-byte `CharacterAndClass`; smallvec 1.15.1 grows by powers of two.
//! `gather_and_sort_combining` sorts disjoint decomposed runs once each. There is
//! no normalized-output String in `split_normalized`.
//!
//! Rust 1.93.1 `core::slice::sort::stable::driftsort_main` uses 4 KiB of stack
//! scratch. Larger runs allocate `max(n - n / 2, min(n, 8_000_000 / 4), small)`
//! u32 entries. At the `StatePath` ceiling (16,384 input bytes, at most 65,536
//! decomposed scalars), this is exactly n. Summing n over disjoint runs is at
//! most the complete decomposition length. The optimize-for-size mergesort
//! uses at most this bound too. Its backing coexists with ICU's buffer.
//!
//! The sum below covers every requested replacement layout, including old/new
//! overlap during realloc, and every sort allocation. It is an upper bound,
//! never a claim that UTF-8 input bytes equal normalization storage. Dependency,
//! data and toolchain pins plus physical allocation census guard this audit.

/// Bound established by the existing fingerprinted NFC decomposition census.
pub(super) const MAX_DECOMPOSITION_SCALARS: usize = 4;

/// Bound cumulative allocator requests for one borrowed normalization pass.
pub fn request_bytes(source_scalars: usize) -> usize {
    const INLINE_SCALARS: usize = 17;
    const FIRST_HEAP_CAPACITY: usize = 32;
    const SORT_STACK_SCALARS: usize = 4096 / core::mem::size_of::<u32>();
    let Some(decomposed) = source_scalars.checked_mul(MAX_DECOMPOSITION_SCALARS) else {
        return usize::MAX;
    };
    if decomposed <= INLINE_SCALARS {
        return 0;
    }
    let Some(capacity) = decomposed.checked_next_power_of_two() else {
        return usize::MAX;
    };
    let Some(growth) = capacity
        .checked_mul(2)
        .and_then(|value| value.checked_sub(FIRST_HEAP_CAPACITY))
        .and_then(|value| value.checked_mul(core::mem::size_of::<u32>()))
    else {
        return usize::MAX;
    };
    let sort_scalars = if decomposed > SORT_STACK_SCALARS {
        decomposed
    } else {
        0
    };
    sort_scalars
        .checked_mul(core::mem::size_of::<u32>())
        .and_then(|sort| growth.checked_add(sort))
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_text_adds_sort_scratch_without_changing_the_name_bound() {
        assert_eq!(request_bytes(4), 0);
        assert_eq!(request_bytes(5), 128);
        assert_eq!(request_bytes(crate::name::MAX_NAME_BYTES), 8064);
        assert_eq!(request_bytes(256), 8064);
        assert_eq!(request_bytes(257), 16256 + 1028 * 4);
        assert_eq!(
            request_bytes(crate::state_path::MAX_STATE_PATH_BYTES),
            524160 + 262144
        );
        assert_eq!(request_bytes(usize::MAX), usize::MAX);
        assert_eq!(request_bytes(usize::MAX / 4), usize::MAX);
    }
}
