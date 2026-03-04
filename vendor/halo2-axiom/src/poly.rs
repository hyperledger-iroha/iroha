//! Contains utilities for performing arithmetic over univariate polynomials in
//! various forms, including computing commitments to them and provably opening
//! the committed polynomials at arbitrary points.

use std::fmt::Debug;
use std::io;
use std::marker::PhantomData;
use std::ops::{Add, Deref, DerefMut, Index, IndexMut, Mul, Range, RangeFrom, RangeFull, Sub};

use crate::arithmetic::parallelize;
use crate::helpers::SerdePrimeField;
use crate::plonk::Assigned;
use crate::SerdeFormat;

#[cfg(feature = "multicore")]
use crate::multicore::{
    IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator, ParallelSlice,
};
use group::ff::{BatchInvert, Field};

/// Generic commitment scheme structures
pub mod commitment;
mod domain;
mod query;
mod strategy;

/// Inner product argument commitment scheme
pub mod ipa;

/// KZG commitment scheme
pub mod kzg;

#[cfg(test)]
mod multiopen_test;

pub use domain::*;
pub use query::{ProverQuery, VerifierQuery};
pub use strategy::{Guard, VerificationStrategy};

/// This is an error that could occur during proving or circuit synthesis.
// TODO: these errors need to be cleaned up
#[derive(Debug)]
pub enum Error {
    /// OpeningProof is not well-formed
    OpeningError,
    /// Caller needs to re-sample a point
    SamplingError,
}

/// The basis over which a polynomial is described.
pub trait Basis: Copy + Debug + Send + Sync {}

/// The polynomial is defined as coefficients
#[derive(Clone, Copy, Debug)]
pub struct Coeff;
impl Basis for Coeff {}

/// The polynomial is defined as coefficients of Lagrange basis polynomials
#[derive(Clone, Copy, Debug)]
pub struct LagrangeCoeff;
impl Basis for LagrangeCoeff {}

/// The polynomial is defined as coefficients of Lagrange basis polynomials in
/// an extended size domain which supports multiplication
#[derive(Clone, Copy, Debug)]
pub struct ExtendedLagrangeCoeff;
impl Basis for ExtendedLagrangeCoeff {}

/// Represents a univariate polynomial defined over a field and a particular
/// basis.
#[derive(Clone, Debug)]
pub struct Polynomial<F, B> {
    pub(crate) values: Vec<F>,
    _marker: PhantomData<B>,
}

impl<F, B> Index<usize> for Polynomial<F, B> {
    type Output = F;

    fn index(&self, index: usize) -> &F {
        self.values.index(index)
    }
}

impl<F, B> IndexMut<usize> for Polynomial<F, B> {
    fn index_mut(&mut self, index: usize) -> &mut F {
        self.values.index_mut(index)
    }
}

impl<F, B> Index<Range<usize>> for Polynomial<F, B> {
    type Output = [F];

    fn index(&self, index: Range<usize>) -> &[F] {
        self.values.index(index)
    }
}

impl<F, B> Index<RangeFrom<usize>> for Polynomial<F, B> {
    type Output = [F];

    fn index(&self, index: RangeFrom<usize>) -> &[F] {
        self.values.index(index)
    }
}

impl<F, B> IndexMut<Range<usize>> for Polynomial<F, B> {
    fn index_mut(&mut self, index: Range<usize>) -> &mut [F] {
        self.values.index_mut(index)
    }
}

impl<F, B> IndexMut<RangeFrom<usize>> for Polynomial<F, B> {
    fn index_mut(&mut self, index: RangeFrom<usize>) -> &mut [F] {
        self.values.index_mut(index)
    }
}

impl<F, B> Index<RangeFull> for Polynomial<F, B> {
    type Output = [F];

    fn index(&self, index: RangeFull) -> &[F] {
        self.values.index(index)
    }
}

impl<F, B> IndexMut<RangeFull> for Polynomial<F, B> {
    fn index_mut(&mut self, index: RangeFull) -> &mut [F] {
        self.values.index_mut(index)
    }
}

impl<F, B> Deref for Polynomial<F, B> {
    type Target = [F];

    fn deref(&self) -> &[F] {
        &self.values[..]
    }
}

impl<F, B> DerefMut for Polynomial<F, B> {
    fn deref_mut(&mut self) -> &mut [F] {
        &mut self.values[..]
    }
}

impl<F, B> Polynomial<F, B> {
    /// Iterate over the values, which are either in coefficient or evaluation
    /// form depending on the basis `B`.
    pub fn iter(&self) -> impl Iterator<Item = &F> {
        self.values.iter()
    }

    /// Iterate over the values mutably, which are either in coefficient or
    /// evaluation form depending on the basis `B`.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut F> {
        self.values.iter_mut()
    }

    /// Gets the size of this polynomial in terms of the number of
    /// coefficients used to describe it.
    pub fn num_coeffs(&self) -> usize {
        self.values.len()
    }
}

impl<F: SerdePrimeField, B> Polynomial<F, B> {
    /// Reads polynomial from buffer using `SerdePrimeField::read`.
    pub(crate) fn read<R: io::Read>(reader: &mut R, format: SerdeFormat) -> Self {
        let mut poly_len = [0u8; 4];
        reader.read_exact(&mut poly_len).unwrap();
        let poly_len = u32::from_be_bytes(poly_len);
        Self {
            values: (0..poly_len)
                .map(|_| F::read(reader, format).unwrap())
                .collect(),
            _marker: PhantomData,
        }
    }

    /// Writes polynomial to buffer using `SerdePrimeField::write`.
    pub(crate) fn write<W: io::Write>(&self, writer: &mut W, format: SerdeFormat) {
        writer
            .write_all(&(self.values.len() as u32).to_be_bytes())
            .unwrap();
        for value in self.values.iter() {
            value.write(writer, format).unwrap();
        }
    }
}

/// Invert each polynomial in place for memory efficiency
pub(crate) fn batch_invert_assigned<F: Field, PA>(
    assigned: Vec<PA>,
) -> Vec<Polynomial<F, LagrangeCoeff>>
where
    PA: Deref<Target = [Assigned<F>]> + Sync,
{
    if assigned.is_empty() {
        return vec![];
    }
    let n = assigned[0].as_ref().len();
    // 1d vector better for memory allocation
    let mut assigned_denominators: Vec<_> = assigned
        .iter()
        .flat_map(|f| f.as_ref().iter().map(|value| value.denominator()))
        .collect();

    assigned_denominators
        .iter_mut()
        // If the denominator is trivial, we can skip it, reducing the
        // size of the batch inversion.
        .filter_map(|d| d.as_mut())
        .batch_invert();

    #[cfg(feature = "multicore")]
    return assigned
        .par_iter()
        .zip(assigned_denominators.par_chunks(n))
        .map(|(poly, inv_denoms)| {
            debug_assert_eq!(inv_denoms.len(), poly.as_ref().len());
            Polynomial {
                values: poly
                    .as_ref()
                    .iter()
                    .zip(inv_denoms.iter())
                    .map(|(a, inv_den)| a.numerator() * inv_den.unwrap_or(F::ONE))
                    .collect(),
                _marker: PhantomData,
            }
        })
        .collect();

    #[cfg(not(feature = "multicore"))]
    return assigned
        .iter()
        .zip(assigned_denominators.chunks(n))
        .map(|(poly, inv_denoms)| {
            debug_assert_eq!(inv_denoms.len(), poly.as_ref().len());
            Polynomial {
                values: poly
                    .as_ref()
                    .iter()
                    .zip(inv_denoms.iter())
                    .map(|(a, inv_den)| a.numerator() * inv_den.unwrap_or(F::ONE))
                    .collect(),
                _marker: PhantomData,
            }
        })
        .collect();
}

impl<F: Field> Polynomial<Assigned<F>, LagrangeCoeff> {
    pub fn invert(
        &self,
        inv_denoms: impl Iterator<Item = F> + ExactSizeIterator,
    ) -> Polynomial<F, LagrangeCoeff> {
        assert_eq!(inv_denoms.len(), self.values.len());
        Polynomial {
            values: self
                .values
                .iter()
                .zip(inv_denoms)
                .map(|(a, inv_den)| a.numerator() * inv_den)
                .collect(),
            _marker: self._marker,
        }
    }
}

impl<'a, F: Field, B: Basis> Add<&'a Polynomial<F, B>> for Polynomial<F, B> {
    type Output = Polynomial<F, B>;

    fn add(mut self, rhs: &'a Polynomial<F, B>) -> Polynomial<F, B> {
        parallelize(&mut self.values, |lhs, start| {
            for (lhs, rhs) in lhs.iter_mut().zip(rhs.values[start..].iter()) {
                *lhs += *rhs;
            }
        });

        self
    }
}

impl<'a, F: Field, B: Basis> Sub<&'a Polynomial<F, B>> for Polynomial<F, B> {
    type Output = Polynomial<F, B>;

    fn sub(mut self, rhs: &'a Polynomial<F, B>) -> Polynomial<F, B> {
        parallelize(&mut self.values, |lhs, start| {
            for (lhs, rhs) in lhs.iter_mut().zip(rhs.values[start..].iter()) {
                *lhs -= *rhs;
            }
        });

        self
    }
}

impl<F: Field> Polynomial<F, LagrangeCoeff> {
    /// Rotates the values in a Lagrange basis polynomial by `Rotation`
    pub fn rotate(&self, rotation: Rotation) -> Polynomial<F, LagrangeCoeff> {
        let mut values = self.values.clone();
        if rotation.0 < 0 {
            values.rotate_right((-rotation.0) as usize);
        } else {
            values.rotate_left(rotation.0 as usize);
        }
        Polynomial {
            values,
            _marker: PhantomData,
        }
    }
}

impl<F: Field, B: Basis> Mul<F> for Polynomial<F, B> {
    type Output = Polynomial<F, B>;

    fn mul(mut self, rhs: F) -> Polynomial<F, B> {
        if rhs == F::ZERO {
            return Polynomial {
                values: vec![F::ZERO; self.len()],
                _marker: PhantomData,
            };
        }
        if rhs == F::ONE {
            return self;
        }

        parallelize(&mut self.values, |lhs, _| {
            for lhs in lhs.iter_mut() {
                *lhs *= rhs;
            }
        });

        self
    }
}

impl<'a, F: Field, B: Basis> Sub<F> for &'a Polynomial<F, B> {
    type Output = Polynomial<F, B>;

    fn sub(self, rhs: F) -> Polynomial<F, B> {
        let mut res = self.clone();
        res.values[0] -= rhs;
        res
    }
}

/// Describes the relative rotation of a vector. Negative numbers represent
/// reverse (leftmost) rotations and positive numbers represent forward (rightmost)
/// rotations. Zero represents no rotation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rotation(pub i32);

impl Rotation {
    /// The current location in the evaluation domain
    pub fn cur() -> Rotation {
        Rotation(0)
    }

    /// The previous location in the evaluation domain
    pub fn prev() -> Rotation {
        Rotation(-1)
    }

    /// The next location in the evaluation domain
    pub fn next() -> Rotation {
        Rotation(1)
    }
}
