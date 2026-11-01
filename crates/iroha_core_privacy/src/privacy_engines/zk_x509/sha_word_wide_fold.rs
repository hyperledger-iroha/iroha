//! Exact widened folding of the original ordered SHA base errors.
//!
//! One public table is shared by all rows/tasks of one challenge-bound provider.
//! Each row owns only four wide sums, four carries and bounded metadata. The
//! native MAIN provider retains the table derived from its bound challenges;
//! generic AIR callers use Horner. No challenge, constraint or term is removed.

use super::*;
use zeroize::Zeroize as _;

/// Immutable powers derived solely from the original validated challenge family.
pub(in crate::privacy_engines::zk_x509) struct ShaWordFoldPowersV1 {
    weights: Box<[[F; SHA_WORD_COPY_LANES_V1]; SHA_WORD_STARK_BASE_ERROR_COUNT_V1]>,
}

impl ShaWordFoldPowersV1 {
    /// Exact one-time public allocation, excluding this pointer-sized owner.
    pub(in crate::privacy_engines::zk_x509) const HEAP_PAYLOAD_BYTES_V1: usize =
        SHA_WORD_STARK_BASE_ERROR_COUNT_V1 * core::mem::size_of::<[F; SHA_WORD_COPY_LANES_V1]>();

    /// Conservative public construction descriptors and small power temporaries.
    /// The exact-capacity conversion reuses the sole allocation; no full table
    /// is ever materialized on the stack or copied into a second allocation.
    pub(in crate::privacy_engines::zk_x509) const CONSTRUCTION_SCRATCH_BYTES_V1: usize =
        core::mem::size_of::<Vec<[F; SHA_WORD_COPY_LANES_V1]>>()
            + 2 * core::mem::size_of::<Self>()
            + 4 * core::mem::size_of::<[F; SHA_WORD_COPY_LANES_V1]>();

    pub(in crate::privacy_engines::zk_x509) fn new_v1(
        challenges: ZkX509ShaWordStarkChallengesV1,
    ) -> Result<Self, ZkX509ShaWordStarkErrorV1> {
        validate_zk_x509_sha_word_stark_challenges_v1(challenges)?;
        let mut weights = Vec::new();
        weights
            .try_reserve_exact(SHA_WORD_STARK_BASE_ERROR_COUNT_V1)
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        if weights.capacity() != SHA_WORD_STARK_BASE_ERROR_COUNT_V1 {
            return Err(ZkX509ShaWordStarkErrorV1::Resource);
        }
        weights.resize(
            SHA_WORD_STARK_BASE_ERROR_COUNT_V1,
            [F::ZERO; SHA_WORD_COPY_LANES_V1],
        );
        let mut powers = [F::ONE; SHA_WORD_COPY_LANES_V1];
        for weight in weights.iter_mut().rev() {
            *weight = powers;
            for (power, challenge) in powers.iter_mut().zip(challenges.base_folding) {
                *power = power.mul(challenge);
            }
        }
        // Length equals capacity, so conversion keeps this allocation. The
        // boxed array pins the extent for every later row and resource check.
        let weights = weights
            .into_boxed_slice()
            .try_into()
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        Ok(Self { weights })
    }

    pub(in crate::privacy_engines::zk_x509) fn row_v1(&self) -> ShaWordWideFoldV1<'_> {
        ShaWordWideFoldV1 {
            powers: self,
            sums: [0; 4],
            carries: [0; 4],
            count: 0,
        }
    }
}

/// Private row-local arithmetic, erased on completion, refusal and unwinding.
pub(in crate::privacy_engines::zk_x509) struct ShaWordWideFoldV1<'a> {
    powers: &'a ShaWordFoldPowersV1,
    sums: [u128; SHA_WORD_COPY_LANES_V1],
    carries: [u64; SHA_WORD_COPY_LANES_V1],
    count: usize,
}

impl ShaWordBaseErrorFoldV1<F> for ShaWordWideFoldV1<'_> {
    fn challenges_v1(&self) -> [F; SHA_WORD_COPY_LANES_V1] {
        const {
            assert!(SHA_WORD_STARK_BASE_ERROR_COUNT_V1 >= 2);
        }
        self.powers.weights[SHA_WORD_STARK_BASE_ERROR_COUNT_V1 - 2]
    }

    #[inline]
    fn push_v1(&mut self, error: F) {
        // An erroneous extra term still reaches the authoritative AIR count
        // refusal. It must not panic or publish a partial folded residue.
        if let Some(weights) = self.powers.weights.get(self.count) {
            for (lane, weight) in weights.iter().enumerate() {
                let product = u128::from(error.0) * u128::from(weight.0);
                let (sum, carried) = self.sums[lane].overflowing_add(product);
                self.sums[lane] = sum;
                self.carries[lane] += u64::from(carried);
            }
        }
        self.count += 1;
    }

    fn finish_v1(self) -> [F; SHA_WORD_COPY_LANES_V1] {
        const {
            assert!(SHA_WORD_COPY_LANES_V1 == 4);
            assert!(SHA_WORD_STARK_BASE_ERROR_COUNT_V1 <= 1024);
        }
        debug_assert_eq!(self.count, SHA_WORD_STARK_BASE_ERROR_COUNT_V1);
        core::array::from_fn(|lane| {
            // 2^128 = -2^32 modulo Goldilocks. At most 440 carries are
            // possible, so the correction itself is canonical and exact.
            F::reduce(self.sums[lane]).sub(F(self.carries[lane] << 32))
        })
    }
}

impl Drop for ShaWordWideFoldV1<'_> {
    fn drop(&mut self) {
        self.sums.zeroize();
        self.carries.zeroize();
        #[cfg(test)]
        ERASURE_OBSERVATION.with(|slot| {
            if let Some((count, all_zero)) = slot.get() {
                slot.set(Some((
                    count + 1,
                    all_zero && self.sums == [0; 4] && self.carries == [0; 4],
                )));
            }
        });
    }
}

#[cfg(test)]
thread_local! {
    static ERASURE_OBSERVATION: core::cell::Cell<Option<(usize, bool)>> = const {
        core::cell::Cell::new(None)
    };
}

#[cfg(test)]
fn fold_challenges_v1() -> ZkX509ShaWordStarkChallengesV1 {
    ZkX509ShaWordStarkChallengesV1 {
        memory: ZkX509WordMemoryChallengesV1 {
            lanes: core::array::from_fn(|lane| {
                let start = 1 + 4 * lane as u64;
                ZkX509WordMemoryLaneChallengesV1 {
                    beta: F(start),
                    address: F(start + 1),
                    value: F(start + 2),
                    is_write: F(start + 3),
                }
            }),
        },
        base_folding: [F(61), F(67), F(71), F(73)],
    }
}

#[test]
fn widened_sha_power_table_owns_one_exact_allocation_across_moves_and_rows() {
    let powers = ShaWordFoldPowersV1::new_v1(fold_challenges_v1()).unwrap();
    let pointer = powers.weights.as_ptr();
    let moved = powers;
    assert_eq!(moved.weights.as_ptr(), pointer);
    assert_eq!(moved.weights.len(), SHA_WORD_STARK_BASE_ERROR_COUNT_V1);
    assert_eq!(
        core::mem::size_of_val(moved.weights.as_ref()),
        ShaWordFoldPowersV1::HEAP_PAYLOAD_BYTES_V1
    );
    assert_eq!(
        core::mem::size_of_val(&moved),
        core::mem::size_of::<usize>()
    );
    assert!(
        ShaWordFoldPowersV1::CONSTRUCTION_SCRATCH_BYTES_V1
            < ShaWordFoldPowersV1::HEAP_PAYLOAD_BYTES_V1 / 10
    );
    for _ in 0..3 {
        let mut row = moved.row_v1();
        for _ in 0..SHA_WORD_STARK_BASE_ERROR_COUNT_V1 {
            row.push_v1(F::ZERO);
        }
        assert_eq!(row.finish_v1(), [F::ZERO; SHA_WORD_COPY_LANES_V1]);
        assert_eq!(moved.weights.as_ptr(), pointer);
    }
}

#[test]
fn widened_sha_fold_matches_horner_and_independent_polynomial_at_field_boundaries() {
    let mut challenges = fold_challenges_v1();
    for gammas in [
        [F(2), F(3), F(5), F(7)],
        core::array::from_fn(|i| F(GOLDILOCKS_MODULUS_V1 - 1 - i as u64)),
    ] {
        challenges.base_folding = gammas;
        let powers = ShaWordFoldPowersV1::new_v1(challenges).unwrap();
        assert_eq!(
            core::mem::size_of_val(&powers),
            core::mem::size_of::<usize>()
        );
        assert_eq!(core::mem::size_of_val(powers.weights.as_ref()), 14_112);
        assert_eq!(ShaWordFoldPowersV1::HEAP_PAYLOAD_BYTES_V1, 14_112);
        assert!(core::mem::size_of::<ShaWordWideFoldV1<'_>>() <= 128);
        assert_eq!(
            powers.weights[SHA_WORD_STARK_BASE_ERROR_COUNT_V1 - 1],
            [F::ONE; 4]
        );
        for pattern in 0..5 {
            let errors: [F; SHA_WORD_STARK_BASE_ERROR_COUNT_V1] =
                core::array::from_fn(|index| match pattern {
                    0 => F::ZERO,
                    1 => F::ONE,
                    2 => F(GOLDILOCKS_MODULUS_V1 - 1),
                    3 => F(if index % 2 == 0 {
                        GOLDILOCKS_MODULUS_V1 - 1
                    } else {
                        1
                    }),
                    _ => F::reduce((index as u128 + 1) * 0xFFFF_FFFF_FFFF_FFC5_u128),
                });
            let mut horner = ShaWordHornerFoldV1::new_v1(gammas);
            let mut wide = powers.row_v1();
            for error in errors {
                horner.push_v1(error);
                wide.push_v1(error);
            }
            let expanded: [F; 4] = core::array::from_fn(|lane| {
                errors
                    .iter()
                    .enumerate()
                    .fold(F::ZERO, |sum, (index, &error)| {
                        sum.add(error.mul(gammas[lane].pow((errors.len() - index - 1) as u128)))
                    })
            });
            assert_eq!(horner.finish_v1(), expanded);
            assert_eq!(wide.finish_v1(), expanded);
        }
    }
}

#[test]
fn widened_sha_fold_rejects_bad_families_and_erases_private_accumulation() {
    let valid = fold_challenges_v1();
    for bad in [F::ZERO, F(GOLDILOCKS_MODULUS_V1), valid.base_folding[1]] {
        let mut challenge = valid;
        challenge.base_folding[0] = bad;
        assert!(ShaWordFoldPowersV1::new_v1(challenge).is_err());
    }
    let powers = ShaWordFoldPowersV1::new_v1(valid).unwrap();
    for unwind in [false, true] {
        ERASURE_OBSERVATION.set(Some((0, true)));
        let result = std::panic::catch_unwind(|| {
            let mut wide = powers.row_v1();
            for _ in 0..17 {
                wide.push_v1(F(GOLDILOCKS_MODULUS_V1 - 1));
            }
            if unwind {
                std::panic::resume_unwind(Box::new(()));
            }
            drop(wide);
        });
        assert_eq!(result.is_err(), unwind);
        assert_eq!(ERASURE_OBSERVATION.replace(None), Some((1, true)));
    }
}
