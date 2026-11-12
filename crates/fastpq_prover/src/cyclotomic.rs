//! Radix-two number-theoretic transform over the Goldilocks field.
//!
//! This is the single CPU transform kernel of the FASTPQ prover: trace replay,
//! coefficient replay, the quotient interpolation and the planner all call it.
//! The verifier performs no transform.
//!
//! A transform reads its stage roots from a table of the powers of the domain
//! generator, so butterflies carry no dependency between each other. Tables
//! are public constants derived from the caller's domain; a small process-wide
//! cache retains the ones in use, bounded by [`ROOT_TABLE_CACHE_MAX_BYTES`].
//! Early stages run block by block so they stay in the first-level cache.
//!
//! Each transform is one sequential job. Callers own parallelism: they run one
//! transform per column or lane on their own worker pool. The result depends
//! only on the input and the domain, never on worker count or cache state.
//!
//! Inputs may be arbitrary `u64` representatives. The first stage normalizes
//! them, so every output is the canonical value of the exact transform.

use std::sync::{Arc, Mutex};

use crate::field::{
    GOLDILOCKS_MODULUS_V1 as GOLDILOCKS_MODULUS, add_base, add_canonical, mul_base, sub_base,
    sub_canonical,
};

/// Rows transformed together before the wider stages sweep the whole column.
/// Thirty-two KiB of words stay inside a first-level data cache.
const BLOCK_ROWS: usize = 1 << 12;
/// Largest root table the cache retains: half of a `2^18`-row domain.
const MAX_CACHED_TABLE_WORDS: usize = 1 << 17;
/// Words retained across all cached root tables.
///
/// The fixed DEEP profile uses the `2^16` trace domain in both directions, the
/// `2^17` coefficient domain, the `2^18` numerator domain in both directions
/// and the short FRI layers, about `3.9 * 2^17` words in total.
const MAX_CACHED_WORDS: usize = 4 * MAX_CACHED_TABLE_WORDS;
/// Largest payload the root-table cache retains for the whole process.
///
/// Producer plans charge this once: the tables are public, but they persist
/// across proof attempts in the same process.
pub(crate) const ROOT_TABLE_CACHE_MAX_BYTES: usize = MAX_CACHED_WORDS * size_of::<u64>();

/// A multiplicative subgroup of order `2^log_size` and its generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    /// Base-two logarithm of the subgroup order.
    pub log_size: u32,
    /// Generator whose powers the transform evaluates at.
    pub generator: u64,
}

impl Domain {
    /// Number of points in the domain.
    pub fn size(&self) -> usize {
        1usize << self.log_size
    }
}

/// Evaluate coefficients at the powers of the domain generator, in place.
///
/// `values[k]` becomes the sum of `values[j] * generator^(j*k)`.
///
/// # Panics
/// Panics when the slice length differs from the domain size.
pub fn fft(values: &mut [u64], domain: Domain) {
    assert_eq!(values.len(), domain.size(), "values must match domain size");
    if values.len() < 2 {
        return;
    }
    let roots = root_powers(domain);
    transform(values, domain.log_size, &roots);
}

/// Interpolate evaluations at the powers of the domain generator, in place.
///
/// This is the forward transform under the inverse generator, scaled by the
/// inverse of the domain size.
///
/// # Panics
/// Panics when the slice length differs from the domain size.
pub fn ifft(values: &mut [u64], domain: Domain) {
    let inverse = Domain {
        log_size: domain.log_size,
        generator: power(domain.generator, GOLDILOCKS_MODULUS - 2),
    };
    fft(values, inverse);
    let scale = power(domain.size() as u64, GOLDILOCKS_MODULUS - 2);
    for value in values.iter_mut() {
        *value = mul_base(*value, scale);
    }
}

/// Decimation-in-time butterflies over bit-reversed input.
///
/// `roots[i]` is `generator^i` for `i < n/2`; the stage of width `len` reads
/// every `n/len`-th entry.
fn transform(values: &mut [u64], log_size: u32, roots: &[u64]) {
    let rows = values.len();
    bit_reverse(values, log_size);
    let block = rows.min(BLOCK_ROWS);
    for chunk in values.chunks_exact_mut(block) {
        normalizing_stage(chunk);
        let mut len = 4;
        while len <= block {
            stage(chunk, len, rows / len, roots);
            len <<= 1;
        }
    }
    let mut len = block << 1;
    while len <= rows {
        stage(values, len, rows / len, roots);
        len <<= 1;
    }
}

/// Width-two stage, whose only root is one. It accepts arbitrary
/// representatives and leaves canonical values for every later stage.
fn normalizing_stage(values: &mut [u64]) {
    for pair in values.as_chunks_mut::<2>().0 {
        let [even, odd] = *pair;
        *pair = [add_base(even, odd), sub_base(even, odd)];
    }
}

/// One stage of width `len` over canonical values.
fn stage(values: &mut [u64], len: usize, stride: usize, roots: &[u64]) {
    let half = len / 2;
    for chunk in values.chunks_exact_mut(len) {
        let (low, high) = chunk.split_at_mut(half);
        for ((even, odd), &root) in low
            .iter_mut()
            .zip(high.iter_mut())
            .zip(roots.iter().step_by(stride))
        {
            let twisted = mul_base(*odd, root);
            let sum = add_canonical(*even, twisted);
            *odd = sub_canonical(*even, twisted);
            *even = sum;
        }
    }
}

/// Swap every index with its `log_size`-bit reversal.
fn bit_reverse(values: &mut [u64], log_size: u32) {
    debug_assert!(log_size > 0 && values.len() == 1 << log_size);
    let shift = usize::BITS - log_size;
    for index in 0..values.len() {
        let partner = index.reverse_bits() >> shift;
        if index < partner {
            values.swap(index, partner);
        }
    }
}

fn power(mut base: u64, mut exponent: u64) -> u64 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mul_base(result, base);
        }
        base = mul_base(base, base);
        exponent >>= 1;
    }
    result
}

/// Most recently used tables last; `words` is the sum of their lengths.
struct RootTables {
    entries: Vec<(Domain, Arc<[u64]>)>,
    words: usize,
}

static ROOT_TABLES: Mutex<RootTables> = Mutex::new(RootTables {
    entries: Vec::new(),
    words: 0,
});

/// The first `n/2` powers of the generator of a domain of at least two points.
///
/// Larger tables than the cache admits are built for the call and dropped
/// with it. A poisoned cache is bypassed; the transform result is unaffected.
fn root_powers(domain: Domain) -> Arc<[u64]> {
    let words = domain.size() / 2;
    let cacheable = words <= MAX_CACHED_TABLE_WORDS;
    if cacheable
        && let Ok(mut cache) = ROOT_TABLES.lock()
        && let Some(position) = cache.entries.iter().position(|(key, _)| *key == domain)
    {
        let entry = cache.entries.remove(position);
        let table = Arc::clone(&entry.1);
        cache.entries.push(entry);
        return table;
    }
    // Build outside the lock: concurrent first uses may each build the same
    // public table, and only one copy is retained.
    let mut table = Vec::with_capacity(words);
    let mut root = 1;
    for _ in 0..words {
        table.push(root);
        root = mul_base(root, domain.generator);
    }
    let table: Arc<[u64]> = table.into();
    if cacheable && let Ok(mut cache) = ROOT_TABLES.lock() {
        cache.retain(domain, &table);
    }
    table
}

impl RootTables {
    /// Retain one table, evicting the least recently used ones over the bound.
    fn retain(&mut self, domain: Domain, table: &Arc<[u64]>) {
        if self.entries.iter().any(|(key, _)| *key == domain) {
            return;
        }
        while !self.entries.is_empty() && self.words + table.len() > MAX_CACHED_WORDS {
            self.words -= self.entries.remove(0).1.len();
        }
        self.words += table.len();
        self.entries.push((domain, Arc::clone(table)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Order-`2^23` root of the fixed DEEP evaluation domain.
    const ROOT: u64 = 0x35c4_528b_4aa6_2eb8;

    fn domain(log_size: u32) -> Domain {
        Domain {
            log_size,
            generator: power(ROOT, 1 << (23 - log_size)),
        }
    }

    fn next_word(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn canonical(state: &mut u64, rows: usize) -> Vec<u64> {
        (0..rows)
            .map(|_| next_word(state) % GOLDILOCKS_MODULUS)
            .collect()
    }

    fn residue(value: u128) -> u64 {
        u64::try_from(value % u128::from(GOLDILOCKS_MODULUS)).expect("residue is below p")
    }

    /// The definition: one independent `u128` reduction per term.
    fn evaluate_directly(values: &[u64], generator: u64) -> Vec<u64> {
        let mut point = 1_u64;
        (0..values.len())
            .map(|_| {
                let mut sum = 0_u64;
                let mut weight = 1_u64;
                for &value in values {
                    let term = residue(u128::from(value) * u128::from(weight));
                    sum = residue(u128::from(sum) + u128::from(term));
                    weight = residue(u128::from(weight) * u128::from(point));
                }
                point = residue(u128::from(point) * u128::from(generator));
                sum
            })
            .collect()
    }

    /// The retired kernel: stage roots multiplied up inside every chunk and an
    /// incremental bit reversal. Kept as an independent oracle at larger sizes.
    fn retired_kernel(values: &mut [u64], domain: Domain) {
        let rows = values.len();
        let mut partner = 0;
        for index in 1..rows {
            let mut bit = rows >> 1;
            while partner & bit != 0 {
                partner ^= bit;
                bit >>= 1;
            }
            partner ^= bit;
            if index < partner {
                values.swap(index, partner);
            }
        }
        let mut len = 2;
        for _ in 0..domain.log_size {
            let half = len / 2;
            let stage_root = power(domain.generator, (rows / len) as u64);
            for chunk in values.chunks_mut(len) {
                let mut twiddle = 1;
                for index in 0..half {
                    let even = chunk[index];
                    let odd = mul_base(chunk[index + half], twiddle);
                    chunk[index] = add_base(even, odd);
                    chunk[index + half] = sub_base(even, odd);
                    twiddle = mul_base(twiddle, stage_root);
                }
            }
            len <<= 1;
        }
    }

    #[test]
    fn forward_transform_matches_the_definition_on_every_small_domain() {
        let mut state = 0x243f_6a88_85a3_08d3;
        for log_size in 0..=8 {
            let domain = domain(log_size);
            let source = canonical(&mut state, domain.size());
            let mut actual = source.clone();
            fft(&mut actual, domain);
            assert_eq!(
                actual,
                evaluate_directly(&source, domain.generator),
                "log size {log_size}"
            );
        }
    }

    #[test]
    fn blocked_stages_match_the_retired_kernel_across_the_block_boundary() {
        let mut state = 0x1319_8a2e_0370_7344;
        // Below, at and above the block width, including both replay domains.
        for log_size in [9, 11, 12, 13, 14, 16, 17] {
            let domain = domain(log_size);
            let source = canonical(&mut state, domain.size());
            let mut actual = source.clone();
            let mut expected = source;
            fft(&mut actual, domain);
            retired_kernel(&mut expected, domain);
            assert_eq!(actual, expected, "log size {log_size}");
        }
    }

    #[test]
    fn inverse_transform_restores_coefficients_and_matches_the_definition() {
        let mut state = 0xa409_3822_299f_31d0;
        for log_size in [0, 1, 2, 5, 12, 13, 16] {
            let domain = domain(log_size);
            let source = canonical(&mut state, domain.size());
            let mut values = source.clone();
            fft(&mut values, domain);
            ifft(&mut values, domain);
            assert_eq!(values, source, "log size {log_size}");
        }
        let domain = domain(6);
        let source = canonical(&mut state, domain.size());
        let mut actual = source.clone();
        ifft(&mut actual, domain);
        let inverse_generator = power(domain.generator, GOLDILOCKS_MODULUS - 2);
        let scale = power(domain.size() as u64, GOLDILOCKS_MODULUS - 2);
        let expected: Vec<_> = evaluate_directly(&source, inverse_generator)
            .into_iter()
            .map(|value| residue(u128::from(value) * u128::from(scale)))
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn arbitrary_representatives_transform_to_canonical_exact_values() {
        let mut state = 0x082e_fa98_ec4e_6c89;
        for log_size in [1, 2, 7, 13] {
            let domain = domain(log_size);
            let mut raw: Vec<u64> = (0..domain.size()).map(|_| next_word(&mut state)).collect();
            // Force the boundary representatives into every size.
            raw[0] = u64::MAX;
            raw[1] = GOLDILOCKS_MODULUS;
            let mut reduced: Vec<u64> = raw.iter().map(|&word| word % GOLDILOCKS_MODULUS).collect();
            fft(&mut raw, domain);
            fft(&mut reduced, domain);
            assert_eq!(raw, reduced, "log size {log_size}");
            assert!(raw.iter().all(|&word| word < GOLDILOCKS_MODULUS));
        }
    }

    #[test]
    fn single_point_domain_is_the_identity_and_length_mismatch_panics() {
        let mut one = [GOLDILOCKS_MODULUS - 1];
        fft(&mut one, domain(0));
        assert_eq!(one, [GOLDILOCKS_MODULUS - 1]);
        ifft(&mut one, domain(0));
        assert_eq!(one, [GOLDILOCKS_MODULUS - 1]);
        let mismatch = std::panic::catch_unwind(|| fft(&mut [1, 2, 3], domain(2)));
        assert!(mismatch.is_err());
    }

    #[test]
    fn result_is_independent_of_cache_state_and_worker_count() {
        let mut state = 0x4528_21e6_38d0_1377;
        let domain = domain(13);
        let source = canonical(&mut state, domain.size());
        let mut cold = source.clone();
        fft(&mut cold, domain);
        // Push more maximal tables through the cache than it retains, so the
        // table used above is evicted unless a concurrent test restores it.
        // Then transform again from one and from four workers.
        for generator in 2..8 {
            let table = root_powers(Domain {
                log_size: 18,
                generator,
            });
            assert_eq!(table.len(), MAX_CACHED_TABLE_WORDS);
        }
        for threads in [1, 4] {
            let outputs = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    use rayon::prelude::*;
                    (0..8)
                        .into_par_iter()
                        .map(|_| {
                            let mut values = source.clone();
                            fft(&mut values, domain);
                            values
                        })
                        .collect::<Vec<_>>()
                });
            assert!(outputs.iter().all(|values| *values == cold));
        }
    }

    #[test]
    fn root_table_cache_never_exceeds_its_declared_payload() {
        let mut cache = RootTables {
            entries: Vec::new(),
            words: 0,
        };
        let table = |words: usize| -> Arc<[u64]> { vec![1; words].into() };
        // Six maximal tables cannot all stay; the oldest ones leave first.
        for generator in 0..6 {
            cache.retain(
                Domain {
                    log_size: 18,
                    generator,
                },
                &table(MAX_CACHED_TABLE_WORDS),
            );
            assert!(cache.words <= MAX_CACHED_WORDS);
            assert_eq!(
                cache.words,
                cache.entries.iter().map(|(_, t)| t.len()).sum::<usize>()
            );
        }
        assert_eq!(cache.entries.len(), 4);
        assert_eq!(cache.entries[0].0.generator, 2);
        // Retaining a present domain changes nothing.
        let before = cache.words;
        cache.retain(
            Domain {
                log_size: 18,
                generator: 5,
            },
            &table(MAX_CACHED_TABLE_WORDS),
        );
        assert_eq!((cache.words, cache.entries.len()), (before, 4));
        assert_eq!(
            ROOT_TABLE_CACHE_MAX_BYTES,
            MAX_CACHED_WORDS * size_of::<u64>()
        );
        // A domain above the cached extent is built for its call only.
        let large = Domain {
            log_size: 19,
            generator: power(ROOT, 16),
        };
        let table = root_powers(large);
        assert_eq!(table.len(), 1 << 18);
        assert!(
            ROOT_TABLES
                .lock()
                .unwrap()
                .entries
                .iter()
                .all(|(key, _)| *key != large)
        );
    }

    #[test]
    fn root_table_holds_consecutive_generator_powers() {
        let domain = domain(10);
        let table = root_powers(domain);
        assert_eq!(table.len(), domain.size() / 2);
        assert_eq!(table[0], 1);
        for pair in table.windows(2) {
            assert_eq!(
                pair[1],
                residue(u128::from(pair[0]) * u128::from(domain.generator))
            );
        }
        // The generator has exact order 2^10, so the half-way power is -1.
        assert_eq!(
            residue(u128::from(table[table.len() - 1]) * u128::from(domain.generator)),
            GOLDILOCKS_MODULUS - 1
        );
    }
}
