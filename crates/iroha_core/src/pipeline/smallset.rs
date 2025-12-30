//! Small-set utilities used in the pipeline for compact u32 key sets.
//!
//! Provides deterministic helpers for sorting, deduplicating, and intersecting
//! small u32 sets. A scalar baseline is always available; when the crate
//! feature `simd` is enabled the helpers transparently switch to portable SIMD
//! implementations with identical results.

use core::cmp::Ordering;

/// Sort a slice of `u32` and compact it in-place by removing duplicates.
/// Returns the number of unique elements; the first `len` items are the
/// deduplicated, sorted values.
#[inline]
pub fn sort_dedup_u32_in_place(slice: &mut [u32]) -> usize {
    #[cfg(feature = "simd")]
    if let Some(len) = simd::sort_dedup_u32(slice) {
        return len;
    }
    scalar::sort_dedup_u32(slice)
}

/// Intersect two sorted `u32` slices and return the intersection (deduplicated).
#[inline]
pub fn intersect_sorted_u32(a: &[u32], b: &[u32]) -> Vec<u32> {
    #[cfg(feature = "simd")]
    if let Some(out) = simd::intersect_sorted_u32(a, b) {
        return out;
    }
    scalar::intersect_sorted_u32(a, b)
}

mod scalar {
    use super::Ordering;

    #[inline]
    pub(super) fn sort_dedup_u32(slice: &mut [u32]) -> usize {
        if slice.len() <= 1 {
            return slice.len();
        }
        slice.sort_unstable();
        let mut write = 1usize;
        let mut last = slice[0];
        for i in 1..slice.len() {
            let v = slice[i];
            if v != last {
                slice[write] = v;
                write += 1;
                last = v;
            }
        }
        write
    }

    #[inline]
    pub(super) fn intersect_sorted_u32(a: &[u32], b: &[u32]) -> Vec<u32> {
        let mut out = Vec::new();
        let mut i = 0usize;
        let mut j = 0usize;
        let mut last: Option<u32> = None;
        while i < a.len() && j < b.len() {
            let va = a[i];
            let vb = b[j];
            match va.cmp(&vb) {
                Ordering::Equal => {
                    if last != Some(va) {
                        out.push(va);
                        last = Some(va);
                    }
                    i += 1;
                    j += 1;
                }
                Ordering::Less => i += 1,
                Ordering::Greater => j += 1,
            }
        }
        out
    }
}

#[cfg(feature = "simd")]
mod simd {
    use core::simd::{LaneCount, Simd, SimdOrd, SimdPartialEq, SupportedLaneCount};

    use super::{Ordering, scalar};

    const LANES: usize = 8;

    #[inline]
    pub(super) fn sort_dedup_u32(slice: &mut [u32]) -> Option<usize>
    where
        LaneCount<LANES>: SupportedLaneCount,
    {
        if slice.len() <= 1 {
            return Some(slice.len());
        }
        if slice.len() > LANES {
            return None;
        }
        bitonic_sort_small(slice);
        let new_len = compact_sorted(slice);
        Some(new_len)
    }

    #[inline]
    pub(super) fn intersect_sorted_u32(a: &[u32], b: &[u32]) -> Option<Vec<u32>>
    where
        LaneCount<LANES>: SupportedLaneCount,
    {
        if a.is_empty() || b.is_empty() {
            return Some(Vec::new());
        }

        let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
        if small.len() < LANES {
            return None;
        }

        let mut result = Vec::new();
        let mut last: Option<u32> = None;
        let mut idx_small = 0usize;
        let mut idx_large = 0usize;

        while idx_small + LANES <= small.len() && idx_large < large.len() {
            let chunk = Simd::<u32, LANES>::from_slice(&small[idx_small..idx_small + LANES]);
            let chunk_arr = chunk.to_array();
            let min = chunk.reduce_min();
            let max = chunk.reduce_max();

            while idx_large < large.len() && large[idx_large] < min {
                idx_large += 1;
            }

            let mut probe = idx_large;
            while probe < large.len() && large[probe] <= max {
                let mask = chunk.simd_eq(Simd::splat(large[probe]));
                let mut bits = mask.to_bitmask() as u32;
                while bits != 0 {
                    let lane = bits.trailing_zeros() as usize;
                    let value = chunk_arr[lane];
                    if last != Some(value) {
                        result.push(value);
                        last = Some(value);
                    }
                    bits &= bits - 1;
                }
                probe += 1;
            }

            if probe > idx_large {
                idx_large = probe;
            }
            idx_small += LANES;
        }

        if idx_small < small.len() && idx_large < large.len() {
            let tail = scalar::intersect_sorted_u32(&small[idx_small..], &large[idx_large..]);
            for value in tail {
                if last != Some(value) {
                    result.push(value);
                    last = Some(value);
                }
            }
        }

        Some(result)
    }

    #[inline]
    fn bitonic_sort_small(slice: &mut [u32])
    where
        LaneCount<LANES>: SupportedLaneCount,
        LaneCount<{ LANES / 2 }>: SupportedLaneCount,
    {
        let len = slice.len();
        let mut buf = [u32::MAX; LANES];
        buf[..len].copy_from_slice(slice);

        stage_pairwise(&mut buf, [0, 2, 4, 6], [1, 3, 5, 7]);
        stage_pairwise(&mut buf, [0, 1, 4, 5], [2, 3, 6, 7]);
        stage_pairwise(&mut buf, [1, 3, 5, 7], [2, 4, 6, 7]);
        stage_pairwise(&mut buf, [0, 1, 2, 3], [4, 5, 6, 7]);
        stage_pairwise(&mut buf, [0, 1, 4, 5], [2, 3, 6, 7]);
        stage_pairwise(&mut buf, [1, 3, 5, 7], [2, 4, 6, 7]);

        slice.copy_from_slice(&buf[..len]);
    }

    #[inline]
    fn stage_pairwise(buf: &mut [u32; LANES], left: [usize; LANES / 2], right: [usize; LANES / 2])
    where
        LaneCount<{ LANES / 2 }>: SupportedLaneCount,
    {
        let lhs = gather(buf, &left);
        let rhs = gather(buf, &right);
        let mins = lhs.simd_min(rhs);
        let maxs = lhs.simd_max(rhs);
        scatter(buf, &left, mins);
        scatter(buf, &right, maxs);
    }

    #[inline]
    fn gather(buf: &[u32; LANES], idx: &[usize; LANES / 2]) -> Simd<u32, { LANES / 2 }>
    where
        LaneCount<{ LANES / 2 }>: SupportedLaneCount,
    {
        let mut values = [0u32; LANES / 2];
        values[0] = buf[idx[0]];
        values[1] = buf[idx[1]];
        values[2] = buf[idx[2]];
        values[3] = buf[idx[3]];
        Simd::from_array(values)
    }

    #[inline]
    fn scatter(buf: &mut [u32; LANES], idx: &[usize; LANES / 2], values: Simd<u32, { LANES / 2 }>)
    where
        LaneCount<{ LANES / 2 }>: SupportedLaneCount,
    {
        let arr = values.to_array();
        buf[idx[0]] = arr[0];
        buf[idx[1]] = arr[1];
        buf[idx[2]] = arr[2];
        buf[idx[3]] = arr[3];
    }

    #[inline]
    fn compact_sorted(slice: &mut [u32]) -> usize
    where
        LaneCount<LANES>: SupportedLaneCount,
    {
        if slice.is_empty() {
            return 0;
        }
        let mut write = 1usize;
        let mut prev = slice[0];
        let mut idx = 1usize;
        while idx + LANES <= slice.len() {
            let chunk = Simd::<u32, LANES>::from_slice(&slice[idx..idx + LANES]);
            let mut prev_arr = [prev; LANES];
            prev_arr[1..].copy_from_slice(&slice[idx..idx + LANES - 1]);
            let prev_simd = Simd::from_array(prev_arr);
            let mask = chunk.simd_ne(prev_simd);
            let mut bits = mask.to_bitmask() as u32;
            let arr = chunk.to_array();
            while bits != 0 {
                let lane = bits.trailing_zeros() as usize;
                let value = arr[lane];
                slice[write] = value;
                write += 1;
                prev = value;
                bits &= bits - 1;
            }
            prev = arr[LANES - 1];
            idx += LANES;
        }
        while idx < slice.len() {
            let value = slice[idx];
            if value != prev {
                slice[write] = value;
                write += 1;
                prev = value;
            }
            idx += 1;
        }
        write
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_dedup_basic() {
        let mut v = vec![3, 1, 2, 2, 1, 4, 3];
        let n = sort_dedup_u32_in_place(&mut v);
        v.truncate(n);
        assert_eq!(v, vec![1, 2, 3, 4]);
    }

    #[test]
    fn sort_dedup_already_sorted() {
        let mut v = vec![1, 2, 3, 3, 4, 4];
        let n = sort_dedup_u32_in_place(&mut v);
        v.truncate(n);
        assert_eq!(v, vec![1, 2, 3, 4]);
    }

    #[test]
    fn intersect_sorted() {
        let a = vec![1, 2, 3, 5, 7, 9];
        let b = vec![2, 3, 4, 9, 10];
        let c = intersect_sorted_u32(&a, &b);
        assert_eq!(c, vec![2, 3, 9]);
    }

    #[cfg(feature = "simd")]
    #[test]
    fn simd_sort_matches_scalar_variants() {
        use super::scalar;

        let mut cases = vec![
            vec![5, 4, 3, 2, 1, 0, 0, 1],
            vec![1, 1, 1, 1, 1],
            vec![7, 2, 7, 3],
            vec![8, 7, 6, 5, 4, 3, 2, 1],
        ];

        for case in cases.iter_mut() {
            let mut scalar_vec = case.clone();
            let scalar_len = scalar::sort_dedup_u32(&mut scalar_vec);
            scalar_vec.truncate(scalar_len);

            let mut simd_vec = case.clone();
            let simd_len = super::simd::sort_dedup_u32(&mut simd_vec)
                .expect("SIMD path should handle <= 8 items");
            simd_vec.truncate(simd_len);

            assert_eq!(simd_vec, scalar_vec);
        }
    }

    #[cfg(feature = "simd")]
    #[test]
    fn simd_intersect_matches_scalar() {
        use super::scalar;

        let a = vec![1, 2, 2, 3, 5, 7, 9, 11];
        let b = vec![0, 2, 3, 4, 9, 11, 12, 13];

        let scalar_out = scalar::intersect_sorted_u32(&a, &b);
        let simd_out = super::simd::intersect_sorted_u32(&a, &b)
            .expect("SIMD path should activate for eight-element chunk");

        assert_eq!(simd_out, scalar_out);
    }
}
