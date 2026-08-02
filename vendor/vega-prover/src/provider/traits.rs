// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Provides traits and extensions for groups, discrete logarithm operations, and homomorphic commitments.
//!
//! This module defines several key traits that abstract over different cryptographic primitives:
//!
//! - [`GroupOps`]: A helper trait bundling common group operations (`Add`, `Sub`, `AddAssign`, `SubAssign`)
//! - [`GroupOpsOwned`]: Extends `GroupOps` to work with references
//! - [`ScalarMulOwned`]: Trait for scalar multiplication with references
//! - [`DlogGroup`]: Core trait for groups supporting discrete logarithm operations
//! - [`DlogGroupExt`]: Extension trait for multi-scalar multiplication (MSM) operations
//!
//! Additionally, the module provides two macros for implementing these traits:
//!
//! - `impl_traits_no_dlog_ext!`: Implements all traits except for `DlogGroupExt`
//! - `impl_traits!`: Implements all traits including `DlogGroupExt`
//!
//! These traits and macros provide a consistent interface for elliptic curve operations
//! and other algebraic structures used throughout the Spartan proof system.
use crate::{
    errors::VegaError,
    traits::{Group, transcript::TranscriptReprTrait},
};
use core::{
    fmt::Debug,
    ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign},
};
use halo2curves::{CurveAffine, serde::SerdeObject};
use num_integer::Integer;
use num_traits::ToPrimitive;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

/// A helper trait for types with a group operation.
pub trait GroupOps<Rhs = Self, Output = Self>:
    Add<Rhs, Output = Output> + Sub<Rhs, Output = Output> + AddAssign<Rhs> + SubAssign<Rhs>
{
}

impl<T, Rhs, Output> GroupOps<Rhs, Output> for T where
    T: Add<Rhs, Output = Output> + Sub<Rhs, Output = Output> + AddAssign<Rhs> + SubAssign<Rhs>
{
}

/// A helper trait for references with a group operation.
pub trait GroupOpsOwned<Rhs = Self, Output = Self>: for<'r> GroupOps<&'r Rhs, Output> {}
impl<T, Rhs, Output> GroupOpsOwned<Rhs, Output> for T where T: for<'r> GroupOps<&'r Rhs, Output> {}

/// A helper trait for types implementing scalar multiplication.
pub trait ScalarMul<Rhs, Output = Self>: Mul<Rhs, Output = Output> + MulAssign<Rhs> {}

impl<T, Rhs, Output> ScalarMul<Rhs, Output> for T where T: Mul<Rhs, Output = Output> + MulAssign<Rhs>
{}

/// A helper trait for references implementing group scalar multiplication.
pub trait ScalarMulOwned<Rhs, Output = Self>: for<'r> ScalarMul<&'r Rhs, Output> {}
impl<T, Rhs, Output> ScalarMulOwned<Rhs, Output> for T where T: for<'r> ScalarMul<&'r Rhs, Output> {}

/// A trait that defines the core discrete logarithm group functionality
pub trait DlogGroup:
    Group
    + TranscriptReprTrait<Self>
    + Serialize
    + for<'de> Deserialize<'de>
    + GroupOps
    + GroupOpsOwned
    + ScalarMul<<Self as Group>::Scalar>
    + ScalarMulOwned<<Self as Group>::Scalar>
{
    /// A type representing preprocessed group element
    type AffineGroupElement: Clone
        + Debug
        + PartialEq
        + Eq
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>
        + TranscriptReprTrait<Self>
        + CurveAffine
        + SerdeObject;

    /// Produce a vector of group elements using a static label
    fn from_label(label: &'static [u8], n: usize) -> Vec<Self::AffineGroupElement>;

    /// Produces a preprocessed element
    fn affine(&self) -> Self::AffineGroupElement;

    /// Batch convert projective points to affine using Montgomery's trick (single inversion).
    fn batch_affine(points: &[Self]) -> Vec<Self::AffineGroupElement> {
        points.iter().map(|p| p.affine()).collect()
    }

    /// Returns a group element from a preprocessed group element
    fn group(p: &Self::AffineGroupElement) -> Self;

    /// Returns an element that is the additive identity of the group
    fn zero() -> Self;

    /// Returns the generator of the group
    fn generator() -> Self;

    /// Returns the affine coordinates (x, y, infinity) for the point
    fn to_coordinates(&self) -> (<Self as Group>::Base, <Self as Group>::Base, bool);

    /// Variable-time mixed addition: self + affine point.
    /// Uses z=1 optimization for the affine operand.
    fn add_affine_vartime(&self, other: &Self::AffineGroupElement) -> Self {
        *self + Self::group(other)
    }
}

/// Extension trait for DlogGroup that provides multi-scalar multiplication operations
pub trait DlogGroupExt: DlogGroup {
    /// A method to compute a multiexponentation
    fn vartime_multiscalar_mul(
        scalars: &[Self::Scalar],
        bases: &[Self::AffineGroupElement],
        use_parallelism_internally: bool,
    ) -> Result<Self, VegaError>;

    /// A method to compute a batch of multiexponentations
    fn batch_vartime_multiscalar_mul(
        scalars: &[Vec<Self::Scalar>],
        bases: &[Self::AffineGroupElement],
    ) -> Result<Vec<Self>, VegaError> {
        scalars
            .par_iter()
            .map(|scalar| Self::vartime_multiscalar_mul(scalar, &bases[..scalar.len()], false))
            .collect::<Result<Vec<_>, _>>()
    }

    /// A method to compute a multiexponentation with small scalars
    fn vartime_multiscalar_mul_small<T: Integer + Into<u64> + Copy + Sync + ToPrimitive>(
        scalars: &[T],
        bases: &[Self::AffineGroupElement],
        use_parallelism_internally: bool,
    ) -> Result<Self, VegaError>;

    /// A method to compute a batch of multiexponentations with small scalars
    fn batch_vartime_multiscalar_mul_small<T: Integer + Into<u64> + Copy + Sync + ToPrimitive>(
        scalars: &[Vec<T>],
        bases: &[Self::AffineGroupElement],
    ) -> Result<Vec<Self>, VegaError> {
        scalars
            .par_iter()
            .map(|scalar| {
                Self::vartime_multiscalar_mul_small(scalar, &bases[..scalar.len()], false)
            })
            .collect::<Result<Vec<_>, _>>()
    }

    /// Shared-weight multi-MSM: all rows use same scalars, different bases.
    /// More efficient than calling vartime_multiscalar_mul repeatedly because
    /// scalar decomposition (to_repr + window extraction) is done once.
    fn vartime_multiscalar_mul_shared_weights(
        scalars: &[Self::Scalar],
        bases_rows: &[&[Self::AffineGroupElement]],
    ) -> Result<Vec<Self>, VegaError>;
}

/// Implements Spartan's traits except DlogGroupExt so that the MSM can be implemented differently
#[macro_export]
macro_rules! impl_traits_no_dlog_ext {
    (
    $name:ident,
    $name_curve:ident,
    $name_curve_affine:ident,
    $order_str:literal,
    $base_str:literal
  ) => {
        impl Group for $name::Point {
            type Base = $name::Base;
            type Scalar = $name::Scalar;

            fn group_params() -> (Self::Base, Self::Base, BigInt, BigInt) {
                let A = $name::Point::a();
                let B = $name::Point::b();
                let order = BigInt::from_str_radix($order_str, 16).unwrap();
                let base = BigInt::from_str_radix($base_str, 16).unwrap();

                (A, B, order, base)
            }
        }

        impl DlogGroup for $name::Point {
            type AffineGroupElement = $name::Affine;

            fn affine(&self) -> Self::AffineGroupElement {
                self.to_affine()
            }

            fn batch_affine(points: &[Self]) -> Vec<Self::AffineGroupElement> {
                let mut affine = vec![$name_curve_affine::identity(); points.len()];
                <Self as Curve>::batch_normalize(points, &mut affine);
                affine
            }

            fn group(p: &Self::AffineGroupElement) -> Self {
                $name::Point::from(*p)
            }

            fn from_label(label: &'static [u8], n: usize) -> Vec<Self::AffineGroupElement> {
                let mut shake = Shake256::default();
                shake.update(label);
                let mut reader = shake.finalize_xof();
                let mut uniform_bytes_vec = Vec::new();
                for _ in 0..n {
                    let mut uniform_bytes = [0u8; 32];
                    reader.read_exact(&mut uniform_bytes).unwrap();
                    uniform_bytes_vec.push(uniform_bytes);
                }
                let gens_proj: Vec<$name_curve> = (0..n)
                    .into_par_iter()
                    .map(|i| {
                        let hash = $name_curve::hash_to_curve("from_uniform_bytes");
                        hash(&uniform_bytes_vec[i])
                    })
                    .collect();

                let num_threads = rayon::current_num_threads();
                if gens_proj.len() > num_threads {
                    let chunk = (gens_proj.len() as f64 / num_threads as f64).ceil() as usize;
                    (0..num_threads)
                        .into_par_iter()
                        .flat_map(|i| {
                            let start = i * chunk;
                            let end = if i == num_threads - 1 {
                                gens_proj.len()
                            } else {
                                core::cmp::min((i + 1) * chunk, gens_proj.len())
                            };
                            if end > start {
                                let mut gens = vec![$name_curve_affine::identity(); end - start];
                                <Self as Curve>::batch_normalize(&gens_proj[start..end], &mut gens);
                                gens
                            } else {
                                vec![]
                            }
                        })
                        .collect()
                } else {
                    let mut gens = vec![$name_curve_affine::identity(); n];
                    <Self as Curve>::batch_normalize(&gens_proj, &mut gens);
                    gens
                }
            }

            fn zero() -> Self {
                $name::Point::identity()
            }

            fn generator() -> Self {
                $name::Point::generator()
            }

            fn to_coordinates(&self) -> (Self::Base, Self::Base, bool) {
                let coordinates = self.affine().coordinates();
                if coordinates.is_some().unwrap_u8() == 1
                    && ($name_curve_affine::identity() != self.affine())
                {
                    (*coordinates.unwrap().x(), *coordinates.unwrap().y(), false)
                } else {
                    (Self::Base::zero(), Self::Base::zero(), true)
                }
            }

            fn add_affine_vartime(&self, other: &Self::AffineGroupElement) -> Self {
                CurveExt::add_mixed_vartime(self, other)
            }
        }

        impl PrimeFieldExt for $name::Scalar {
            fn from_uniform(bytes: &[u8]) -> Self {
                let bytes_arr: [u8; 64] = bytes.try_into().unwrap();
                $name::Scalar::from_uniform_bytes(&bytes_arr)
            }
        }

        impl<G: Group> TranscriptReprTrait<G> for $name::Scalar {
            fn to_transcript_bytes(&self) -> Vec<u8> {
                self.to_bytes().into_iter().rev().collect()
            }
        }

        impl<G: DlogGroup> TranscriptReprTrait<G> for $name::Affine {
            fn to_transcript_bytes(&self) -> Vec<u8> {
                let coords = self.coordinates().unwrap();
                let x_bytes = coords.x().to_bytes().into_iter();
                let y_bytes = coords.y().to_bytes().into_iter();
                x_bytes.rev().chain(y_bytes.rev()).collect()
            }
        }

        impl<G: DlogGroup> TranscriptReprTrait<G> for $name::Point {
            fn to_transcript_bytes(&self) -> Vec<u8> {
                let affine = self.affine();
                let coords = affine.coordinates().unwrap();
                let x_bytes = coords.x().to_bytes().into_iter();
                let y_bytes = coords.y().to_bytes().into_iter();
                x_bytes.rev().chain(y_bytes.rev()).collect()
            }
        }
    };
}

/// Implements Spartan's traits
#[macro_export]
macro_rules! impl_traits {
    (
    $name:ident,
    $name_curve:ident,
    $name_curve_affine:ident,
    $order_str:literal,
    $base_str:literal
  ) => {
        $crate::impl_traits_no_dlog_ext!(
            $name,
            $name_curve,
            $name_curve_affine,
            $order_str,
            $base_str
        );

        impl DlogGroupExt for $name::Point {
            fn vartime_multiscalar_mul(
                scalars: &[Self::Scalar],
                bases: &[Self::AffineGroupElement],
                use_parallelism_internally: bool,
            ) -> Result<Self, $crate::errors::VegaError> {
                msm(scalars, bases, use_parallelism_internally)
            }

            fn vartime_multiscalar_mul_small<T: Integer + Into<u64> + Copy + Sync + ToPrimitive>(
                scalars: &[T],
                bases: &[Self::AffineGroupElement],
                use_parallelism_internally: bool,
            ) -> Result<Self, $crate::errors::VegaError> {
                msm_small(scalars, bases, use_parallelism_internally)
            }

            fn vartime_multiscalar_mul_shared_weights(
                scalars: &[Self::Scalar],
                bases_rows: &[&[Self::AffineGroupElement]],
            ) -> Result<Vec<Self>, $crate::errors::VegaError> {
                msm_shared_weights(scalars, bases_rows)
            }
        }
    };
}
