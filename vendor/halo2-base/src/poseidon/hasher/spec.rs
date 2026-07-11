use crate::{
    ff::{FromUniformBytes, PrimeField},
    poseidon::hasher::mds::*,
};

use getset::{CopyGetters, Getters};
use poseidon_rs::poseidon::primitives::Spec as PoseidonSpec; // trait
use std::marker::PhantomData;

// struct so we can use PoseidonSpec trait to generate round constants and MDS matrix
#[derive(Debug)]
pub(crate) struct Poseidon128Pow5Gen<
    F: PrimeField,
    const T: usize,
    const RATE: usize,
    const R_F: usize,
    const R_P: usize,
    const SECURE_MDS: usize,
> {
    _marker: PhantomData<F>,
}

impl<
        F: PrimeField,
        const T: usize,
        const RATE: usize,
        const R_F: usize,
        const R_P: usize,
        const SECURE_MDS: usize,
    > PoseidonSpec<F, T, RATE> for Poseidon128Pow5Gen<F, T, RATE, R_F, R_P, SECURE_MDS>
{
    fn full_rounds() -> usize {
        R_F
    }

    fn partial_rounds() -> usize {
        R_P
    }

    fn sbox(val: F) -> F {
        val.pow_vartime([5])
    }

    // see "Avoiding insecure matrices" in Section 2.3 of https://eprint.iacr.org/2019/458.pdf
    // most Specs used in practice have SECURE_MDS = 0
    fn secure_mds() -> usize {
        SECURE_MDS
    }
}

// We use the optimized Poseidon implementation described in Supplementary Material Section B of https://eprint.iacr.org/2019/458.pdf
// This involves some further computation of optimized constants and sparse MDS matrices beyond what the Scroll PoseidonSpec generates
// The implementation below is adapted from https://github.com/privacy-scaling-explorations/poseidon

/// `OptimizedPoseidonSpec` holds construction parameters as well as constants that are used in
/// permutation step.
#[derive(Debug, Clone, Getters, CopyGetters)]
pub struct OptimizedPoseidonSpec<F: PrimeField, const T: usize, const RATE: usize> {
    /// Number of full rounds
    #[getset(get_copy = "pub")]
    pub(crate) r_f: usize,
    /// MDS matrices
    #[getset(get = "pub")]
    pub(crate) mds_matrices: MDSMatrices<F, T, RATE>,
    /// Round constants
    #[getset(get = "pub")]
    pub(crate) constants: OptimizedConstants<F, T>,
}

/// `OptimizedConstants` has round constants that are added each round. While
/// full rounds has T sized constants there is a single constant for each
/// partial round
#[derive(Debug, Clone, Getters)]
pub struct OptimizedConstants<F: PrimeField, const T: usize> {
    /// start
    #[getset(get = "pub")]
    pub(crate) start: Vec<[F; T]>,
    /// partial
    #[getset(get = "pub")]
    pub(crate) partial: Vec<F>,
    /// end
    #[getset(get = "pub")]
    pub(crate) end: Vec<[F; T]>,
}

impl<F: PrimeField, const T: usize, const RATE: usize> OptimizedPoseidonSpec<F, T, RATE> {
    /// Generate new spec with specific number of full and partial rounds. `SECURE_MDS` is usually 0, but may need to be specified because insecure matrices may sometimes be generated
    pub fn new<const R_F: usize, const R_P: usize, const SECURE_MDS: usize>() -> Self
    where
        F: FromUniformBytes<64> + Ord,
    {
        let (round_constants, mds, mds_inv) =
            Poseidon128Pow5Gen::<F, T, RATE, R_F, R_P, SECURE_MDS>::constants();
        let mds = MDSMatrix(mds);
        let inverse_mds = MDSMatrix(mds_inv);

        let constants =
            Self::calculate_optimized_constants(R_F, R_P, round_constants, &inverse_mds);
        let (sparse_matrices, pre_sparse_mds) = Self::calculate_sparse_matrices(R_P, &mds);

        Self {
            r_f: R_F,
            constants,
            mds_matrices: MDSMatrices { mds, sparse_matrices, pre_sparse_mds },
        }
    }

    fn calculate_optimized_constants(
        r_f: usize,
        r_p: usize,
        constants: Vec<[F; T]>,
        inverse_mds: &MDSMatrix<F, T, RATE>,
    ) -> OptimizedConstants<F, T> {
        let (number_of_rounds, r_f_half) = (r_f + r_p, r_f / 2);
        assert_eq!(constants.len(), number_of_rounds);

        // Calculate optimized constants for first half of the full rounds
        let mut constants_start: Vec<[F; T]> = vec![[F::ZERO; T]; r_f_half];
        constants_start[0] = constants[0];
        for (optimized, constants) in
            constants_start.iter_mut().skip(1).zip(constants.iter().skip(1))
        {
            *optimized = inverse_mds.mul_vector(constants);
        }

        // Calculate constants for partial rounds
        let mut acc = constants[r_f_half + r_p];
        let mut constants_partial = vec![F::ZERO; r_p];
        for (optimized, constants) in constants_partial
            .iter_mut()
            .rev()
            .zip(constants.iter().skip(r_f_half).rev().skip(r_f_half))
        {
            let mut tmp = inverse_mds.mul_vector(&acc);
            *optimized = tmp[0];

            tmp[0] = F::ZERO;
            for ((acc, tmp), constant) in acc.iter_mut().zip(tmp).zip(constants.iter()) {
                *acc = tmp + constant
            }
        }
        constants_start.push(inverse_mds.mul_vector(&acc));

        // Calculate optimized constants for ending half of the full rounds
        let mut constants_end: Vec<[F; T]> = vec![[F::ZERO; T]; r_f_half - 1];
        for (optimized, constants) in
            constants_end.iter_mut().zip(constants.iter().skip(r_f_half + r_p + 1))
        {
            *optimized = inverse_mds.mul_vector(constants);
        }

        OptimizedConstants {
            start: constants_start,
            partial: constants_partial,
            end: constants_end,
        }
    }

    fn calculate_sparse_matrices(
        r_p: usize,
        mds: &MDSMatrix<F, T, RATE>,
    ) -> (Vec<SparseMDSMatrix<F, T, RATE>>, MDSMatrix<F, T, RATE>) {
        let mds = mds.transpose();
        let mut acc = mds.clone();
        let mut sparse_matrices = (0..r_p)
            .map(|_| {
                let (m_prime, m_prime_prime) = acc.factorise();
                acc = mds.mul(&m_prime);
                m_prime_prime
            })
            .collect::<Vec<SparseMDSMatrix<F, T, RATE>>>();

        sparse_matrices.reverse();
        (sparse_matrices, acc.transpose())
    }
}
