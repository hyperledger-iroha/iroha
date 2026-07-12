#![allow(clippy::needless_range_loop)]
use getset::Getters;

use crate::ff::PrimeField;

/// The type used to hold the MDS matrix
pub(crate) type Mds<F, const T: usize> = [[F; T]; T];

/// `MDSMatrices` holds the MDS matrix as well as transition matrix which is
/// also called `pre_sparse_mds` and sparse matrices that enables us to reduce
/// number of multiplications in apply MDS step
#[derive(Debug, Clone, Getters)]
pub struct MDSMatrices<F: PrimeField, const T: usize, const RATE: usize> {
    /// MDS matrix
    #[getset(get = "pub")]
    pub(crate) mds: MDSMatrix<F, T, RATE>,
    /// Transition matrix
    #[getset(get = "pub")]
    pub(crate) pre_sparse_mds: MDSMatrix<F, T, RATE>,
    /// Sparse matrices
    #[getset(get = "pub")]
    pub(crate) sparse_matrices: Vec<SparseMDSMatrix<F, T, RATE>>,
}

/// `SparseMDSMatrix` are in `[row], [hat | identity]` form and used in linear
/// layer of partial rounds instead of the original MDS
#[derive(Debug, Clone, Getters)]
pub struct SparseMDSMatrix<F: PrimeField, const T: usize, const RATE: usize> {
    /// row
    #[getset(get = "pub")]
    pub(crate) row: [F; T],
    /// column transpose
    #[getset(get = "pub")]
    pub(crate) col_hat: [F; RATE],
}

/// `MDSMatrix` is applied to `State` to achive linear layer of Poseidon
#[derive(Clone, Debug)]
pub struct MDSMatrix<F, const T: usize, const RATE: usize>(pub(crate) Mds<F, T>);

impl<F, const T: usize, const RATE: usize> AsRef<Mds<F, T>> for MDSMatrix<F, T, RATE> {
    fn as_ref(&self) -> &Mds<F, T> {
        &self.0
    }
}

impl<F: PrimeField, const T: usize, const RATE: usize> MDSMatrix<F, T, RATE> {
    pub(crate) fn mul_vector(&self, v: &[F; T]) -> [F; T] {
        let mut res = [F::ZERO; T];
        for i in 0..T {
            for j in 0..T {
                res[i] += self.0[i][j] * v[j];
            }
        }
        res
    }

    pub(crate) fn identity() -> Mds<F, T> {
        let mut mds = [[F::ZERO; T]; T];
        for i in 0..T {
            mds[i][i] = F::ONE;
        }
        mds
    }

    /// Multiplies two MDS matrices. Used in sparse matrix calculations
    pub(crate) fn mul(&self, other: &Self) -> Self {
        let mut res = [[F::ZERO; T]; T];
        for i in 0..T {
            for j in 0..T {
                for k in 0..T {
                    res[i][j] += self.0[i][k] * other.0[k][j];
                }
            }
        }
        Self(res)
    }

    pub(crate) fn transpose(&self) -> Self {
        let mut res = [[F::ZERO; T]; T];
        for i in 0..T {
            for j in 0..T {
                res[i][j] = self.0[j][i];
            }
        }
        Self(res)
    }

    pub(crate) fn determinant<const N: usize>(m: [[F; N]; N]) -> F {
        let mut res = F::ONE;
        let mut m = m;
        for i in 0..N {
            let mut pivot = i;
            while m[pivot][i] == F::ZERO {
                pivot += 1;
                assert!(pivot < N, "matrix is not invertible");
            }
            if pivot != i {
                res = -res;
                m.swap(pivot, i);
            }
            res *= m[i][i];
            let inv = m[i][i].invert().unwrap();
            for j in i + 1..N {
                let factor = m[j][i] * inv;
                for k in i + 1..N {
                    m[j][k] -= m[i][k] * factor;
                }
            }
        }
        res
    }

    /// See Section B in Supplementary Material https://eprint.iacr.org/2019/458.pdf
    /// Factorises an MDS matrix `M` into `M'` and `M''` where `M = M' *  M''`.
    /// Resulted `M''` matrices are the sparse ones while `M'` will contribute
    /// to the accumulator of the process
    pub(crate) fn factorise(&self) -> (Self, SparseMDSMatrix<F, T, RATE>) {
        assert_eq!(RATE + 1, T);
        // Given `(t-1 * t-1)` MDS matrix called `hat` constructs the `t * t` matrix in
        // form `[[1 | 0], [0 | m]]`, ie `hat` is the right bottom sub-matrix
        let prime = |hat: Mds<F, RATE>| -> Self {
            let mut prime = Self::identity();
            for (prime_row, hat_row) in prime.iter_mut().skip(1).zip(hat.iter()) {
                for (el_prime, el_hat) in prime_row.iter_mut().skip(1).zip(hat_row.iter()) {
                    *el_prime = *el_hat;
                }
            }
            Self(prime)
        };

        // Given `(t-1)` sized `w_hat` vector constructs the matrix in form
        // `[[m_0_0 | m_0_i], [w_hat | identity]]`
        let prime_prime = |w_hat: [F; RATE]| -> Mds<F, T> {
            let mut prime_prime = Self::identity();
            prime_prime[0] = self.0[0];
            for (row, w) in prime_prime.iter_mut().skip(1).zip(w_hat.iter()) {
                row[0] = *w
            }
            prime_prime
        };

        let w = self.0.iter().skip(1).map(|row| row[0]).collect::<Vec<_>>();
        // m_hat is the `(t-1 * t-1)` right bottom sub-matrix of m := self.0
        let mut m_hat = [[F::ZERO; RATE]; RATE];
        for i in 0..RATE {
            for j in 0..RATE {
                m_hat[i][j] = self.0[i + 1][j + 1];
            }
        }
        // w_hat = m_hat^{-1} * w, where m_hat^{-1} is matrix inverse and * is matrix mult
        // we avoid computing m_hat^{-1} explicitly by using Cramer's rule: https://en.wikipedia.org/wiki/Cramer%27s_rule
        let mut w_hat = [F::ZERO; RATE];
        let det = Self::determinant(m_hat);
        let det_inv = Option::<F>::from(det.invert()).expect("matrix is not invertible");
        for j in 0..RATE {
            let mut m_hat_j = m_hat;
            for i in 0..RATE {
                m_hat_j[i][j] = w[i];
            }
            w_hat[j] = Self::determinant(m_hat_j) * det_inv;
        }
        let m_prime = prime(m_hat);
        let m_prime_prime = prime_prime(w_hat);
        // row = first row of m_prime_prime.transpose() = first column of m_prime_prime
        let row: [F; T] =
            m_prime_prime.iter().map(|row| row[0]).collect::<Vec<_>>().try_into().unwrap();
        // col_hat = first column of m_prime_prime.transpose() without first element = first row of m_prime_prime without first element
        let col_hat: [F; RATE] = m_prime_prime[0][1..].try_into().unwrap();
        (m_prime, SparseMDSMatrix { row, col_hat })
    }
}
