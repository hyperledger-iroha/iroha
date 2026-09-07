//! Bounded reuse of exact mixed-trace and AIR-row leaf hashes in one verification.

use std::collections::BTreeMap;

use crate::{
    GoldilocksFp4V1, Result,
    backend::{hash_air_trace_row, hash_lde_chunk_fp4},
};
use fastpq_isi::GoldilocksDigest384V1;

/// One mixed-trace chunk for each declared default query.
const MAX_CACHED_LDE_LEAVES: usize = super::DEFAULT_MAX_VERIFY_QUERIES;
/// One current and one next AIR row for each declared default query.
const MAX_CACHED_AIR_LEAVES: usize = 2 * super::DEFAULT_MAX_VERIFY_QUERIES;

/// Cache a fixed leaf-hash role using every input word as part of the key.
///
/// The hasher is fixed at construction and cannot change between lookups. Keys
/// borrow immutable proof data and compare its entire contents, never an address
/// or short fingerprint. Saturation skips insertion while hashing continues.
struct BorrowedLeafCache<'proof, Value: Ord> {
    leaves: BTreeMap<(usize, &'proof [Value]), GoldilocksDigest384V1>,
    max_entries: usize,
    hasher: fn(usize, &[Value]) -> Result<GoldilocksDigest384V1>,
    #[cfg(test)]
    hash_computations: usize,
}

impl<'proof, Value: Ord> BorrowedLeafCache<'proof, Value> {
    fn new(
        max_entries: usize,
        hasher: fn(usize, &[Value]) -> Result<GoldilocksDigest384V1>,
    ) -> Self {
        Self {
            leaves: BTreeMap::new(),
            max_entries,
            hasher,
            #[cfg(test)]
            hash_computations: 0,
        }
    }

    fn hash(&mut self, index: usize, values: &'proof [Value]) -> Result<GoldilocksDigest384V1> {
        let key = (index, values);
        if let Some(&digest) = self.leaves.get(&key) {
            return Ok(digest);
        }
        let digest = (self.hasher)(index, values)?;
        #[cfg(test)]
        {
            self.hash_computations += 1;
        }
        if self.leaves.len() < self.max_entries {
            self.leaves.insert(key, digest);
        }
        Ok(digest)
    }
}

/// Reuse only mixed-trace Fp4 leaf computations in a single verifier call.
pub(super) struct LdeLeafCache<'proof>(BorrowedLeafCache<'proof, GoldilocksFp4V1>);

impl Default for LdeLeafCache<'_> {
    fn default() -> Self {
        Self(BorrowedLeafCache::new(
            MAX_CACHED_LDE_LEAVES,
            hash_lde_chunk_fp4,
        ))
    }
}

impl<'proof> LdeLeafCache<'proof> {
    /// Compute the exact mixed-trace leaf digest, borrowing its input for reuse.
    ///
    /// # Errors
    /// Returns the normal hash error; errors never insert an entry.
    pub(super) fn hash(
        &mut self,
        index: usize,
        values: &'proof [GoldilocksFp4V1],
    ) -> Result<GoldilocksDigest384V1> {
        self.0.hash(index, values)
    }

    #[cfg(test)]
    fn with_test_limit(limit: usize) -> Self {
        Self(BorrowedLeafCache::new(
            limit.min(MAX_CACHED_LDE_LEAVES),
            hash_lde_chunk_fp4,
        ))
    }
}

/// Reuse only full AIR trace-row leaf computations in a single verifier call.
pub(super) struct AirTraceLeafCache<'proof>(BorrowedLeafCache<'proof, u64>);

impl Default for AirTraceLeafCache<'_> {
    fn default() -> Self {
        Self(BorrowedLeafCache::new(
            MAX_CACHED_AIR_LEAVES,
            hash_air_trace_row,
        ))
    }
}

impl<'proof> AirTraceLeafCache<'proof> {
    /// Compute the exact AIR-row digest, borrowing its input for reuse.
    ///
    /// # Errors
    /// Returns the normal row-hash error; errors never insert an entry.
    pub(super) fn hash(
        &mut self,
        index: usize,
        values: &'proof [u64],
    ) -> Result<GoldilocksDigest384V1> {
        self.0.hash(index, values)
    }

    #[cfg(test)]
    fn with_test_limit(limit: usize) -> Self {
        Self(BorrowedLeafCache::new(
            limit.min(MAX_CACHED_AIR_LEAVES),
            hash_air_trace_row,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    fn chunk() -> Vec<GoldilocksFp4V1> {
        (0..3)
            .map(|value| GoldilocksFp4V1::new([value + 1, 7, 11, 13]).unwrap())
            .collect()
    }

    #[test]
    fn repeated_equal_chunks_at_one_index_hash_once_without_copying_input() {
        let first = chunk();
        let separate_allocation = first.clone();
        assert_ne!(first.as_ptr(), separate_allocation.as_ptr());
        let mut cache = LdeLeafCache::default();
        assert_eq!(cache.0.max_entries, 136);
        let expected = hash_lde_chunk_fp4(5, &first).unwrap();
        for values in [&first, &separate_allocation, &first] {
            assert_eq!(cache.hash(5, values).unwrap(), expected);
        }
        assert_eq!(cache.0.leaves.len(), 1);
        assert_eq!(cache.0.hash_computations, 1);
        let ((_, retained), _) = cache.0.leaves.first_key_value().unwrap();
        assert_eq!(retained.as_ptr(), first.as_ptr());
    }

    #[test]
    fn every_changed_coefficient_at_the_same_index_computes_a_fresh_hash() {
        let original = chunk();
        let mutations = (0..original.len())
            .flat_map(|value| {
                let original = &original;
                (0..4).map(move |lane| {
                    let mut changed = original.clone();
                    let mut coefficients = changed[value].coefficients();
                    coefficients[lane] += 1;
                    changed[value] = GoldilocksFp4V1::new(coefficients).unwrap();
                    changed
                })
            })
            .collect::<Vec<_>>();
        let mut cache = LdeLeafCache::default();
        let initial = cache.hash(2, &original).unwrap();
        for values in &mutations {
            let actual = cache.hash(2, values).unwrap();
            assert_eq!(actual, hash_lde_chunk_fp4(2, values).unwrap());
            assert_ne!(actual, initial);
        }
        assert_eq!(cache.0.hash_computations, 1 + original.len() * 4);
        assert_eq!(cache.0.leaves.len(), cache.0.hash_computations);
        assert_eq!(cache.hash(2, &original).unwrap(), initial);
        assert_eq!(cache.0.hash_computations, 1 + original.len() * 4);
    }

    #[test]
    fn leaf_indices_and_complete_chunk_lengths_are_part_of_the_key() {
        let values = chunk();
        let mut cache = LdeLeafCache::default();
        let original = cache.hash(0, &values).unwrap();
        let other_index = cache.hash(1, &values).unwrap();
        let truncated = cache.hash(0, &values[..2]).unwrap();
        let empty = cache.hash(0, &[]).unwrap();
        assert_eq!(other_index, hash_lde_chunk_fp4(1, &values).unwrap());
        assert_eq!(truncated, hash_lde_chunk_fp4(0, &values[..2]).unwrap());
        assert_eq!(empty, hash_lde_chunk_fp4(0, &[]).unwrap());
        assert_ne!(original, other_index);
        assert_ne!(original, truncated);
        assert_ne!(original, empty);
        assert_eq!(cache.0.hash_computations, 4);
    }

    #[test]
    fn saturated_and_disabled_caches_preserve_hashes_and_existing_entries() {
        let values = chunk();
        for limit in [0, 1] {
            let mut cache = LdeLeafCache::with_test_limit(limit);
            for index in [0, 1, 1, 0] {
                assert_eq!(
                    cache.hash(index, &values).unwrap(),
                    hash_lde_chunk_fp4(index, &values).unwrap()
                );
            }
            assert_eq!(cache.0.leaves.len(), limit);
            assert_eq!(cache.0.hash_computations, if limit == 0 { 4 } else { 3 });
        }
    }

    #[test]
    fn malformed_coefficients_cannot_hit_or_enter_a_valid_cache_entry() {
        let values = chunk();
        let invalid = [GoldilocksFp4V1::from_coefficients_unchecked_for_test([
            1,
            7,
            11,
            crate::field::GOLDILOCKS_MODULUS_V1,
        ])];
        let mut cache = LdeLeafCache::default();
        cache.hash(0, &values[..1]).unwrap();
        assert!(matches!(
            cache.hash(0, &invalid),
            Err(Error::NonCanonicalGoldilocksElement { .. })
        ));
        assert_eq!(cache.0.leaves.len(), 1);
        assert_eq!(cache.0.hash_computations, 1);
    }
    #[test]
    fn overlapping_current_and_next_air_rows_hash_once_per_exact_row() {
        let rows = (0..16)
            .map(|index| vec![index, index + 17, index + 37])
            .collect::<Vec<_>>();
        let separate_copies = rows.clone();
        let mut cache = AirTraceLeafCache::default();
        assert_eq!(cache.0.max_entries, 272);
        for current in 0..16 {
            let next = (current + 8) % 16;
            assert_eq!(
                cache.hash(current, &rows[current]).unwrap(),
                hash_air_trace_row(current, &rows[current]).unwrap()
            );
            assert_eq!(
                cache.hash(next, &separate_copies[next]).unwrap(),
                hash_air_trace_row(next, &rows[next]).unwrap()
            );
        }
        assert_eq!(
            cache.0.hash_computations, 16,
            "32 openings share 16 complete row hashes"
        );
        assert_eq!(cache.0.leaves.len(), 16);
    }

    #[test]
    fn changing_any_air_word_at_a_shared_index_never_reuses_the_original_hash() {
        let original = vec![0, 1, 7, 13, crate::GOLDILOCKS_MODULUS_V1 - 1];
        let changed = (0..original.len())
            .map(|word| {
                let mut row = original.clone();
                row[word] = if row[word] == crate::GOLDILOCKS_MODULUS_V1 - 1 {
                    0
                } else {
                    row[word] + 1
                };
                row
            })
            .collect::<Vec<_>>();
        let mut cache = AirTraceLeafCache::default();
        let original_hash = cache.hash(5, &original).unwrap();
        for row in &changed {
            let actual = cache.hash(5, row).unwrap();
            assert_eq!(actual, hash_air_trace_row(5, row).unwrap());
            assert_ne!(actual, original_hash);
        }
        assert_eq!(cache.0.hash_computations, 1 + original.len());
        assert_eq!(cache.hash(5, &original).unwrap(), original_hash);
        assert_eq!(cache.0.hash_computations, 1 + original.len());
    }

    #[test]
    fn air_leaf_indices_and_complete_row_lengths_are_distinct_cache_keys() {
        let row = [1, 2, 3, 4];
        let mut cache = AirTraceLeafCache::default();
        let original = cache.hash(0, &row).unwrap();
        for (index, values) in [(1, row.as_slice()), (0, &row[..3]), (0, &[][..])] {
            let actual = cache.hash(index, values).unwrap();
            assert_eq!(actual, hash_air_trace_row(index, values).unwrap());
            assert_ne!(original, actual);
        }
        assert_eq!(cache.0.hash_computations, 4);
    }

    #[test]
    fn air_cache_rejects_each_noncanonical_word_without_insertion() {
        let original = [1, 2, 3, 4];
        let invalid = (0..original.len())
            .map(|word| {
                let mut row = original;
                row[word] = crate::GOLDILOCKS_MODULUS_V1;
                row
            })
            .collect::<Vec<_>>();
        let mut cache = AirTraceLeafCache::default();
        let expected = cache.hash(0, &original).unwrap();
        for row in &invalid {
            assert!(matches!(
                cache.hash(0, row),
                Err(Error::NonCanonicalGoldilocksElement { .. })
            ));
        }
        assert_eq!(cache.0.leaves.len(), 1);
        assert_eq!(cache.0.hash_computations, 1);
        assert_eq!(cache.hash(0, &original).unwrap(), expected);
    }

    #[test]
    fn saturated_and_disabled_air_caches_keep_exact_hashes_and_old_entries() {
        let row = [1, 2, 3];
        for limit in [0, 1] {
            let mut cache = AirTraceLeafCache::with_test_limit(limit);
            for index in [0, 1, 1, 0] {
                assert_eq!(
                    cache.hash(index, &row).unwrap(),
                    hash_air_trace_row(index, &row).unwrap()
                );
            }
            assert_eq!(cache.0.leaves.len(), limit);
            assert_eq!(cache.0.hash_computations, if limit == 0 { 4 } else { 3 });
        }
    }

    #[test]
    fn typed_leaf_caches_isolate_roles_even_for_identical_serialized_values() {
        let fp4 = [GoldilocksFp4V1::new([1, 2, 3, 4]).unwrap()];
        let base = [1_u64, 2, 3, 4];
        let mut lde = LdeLeafCache::default();
        let mut air = AirTraceLeafCache::default();
        let lde_hash = lde.hash(0, &fp4).unwrap();
        let air_hash = air.hash(0, &base).unwrap();
        assert_ne!(lde_hash, air_hash);
        assert_eq!(lde.hash(0, &fp4).unwrap(), lde_hash);
        assert_eq!(air.hash(0, &base).unwrap(), air_hash);
        assert_eq!(lde.0.hash_computations, 1);
        assert_eq!(air.0.hash_computations, 1);
    }
}
