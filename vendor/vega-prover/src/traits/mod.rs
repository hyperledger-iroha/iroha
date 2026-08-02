// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module defines various traits required by the users of the library to implement.
use core::fmt::Debug;
use ff::{PrimeField, PrimeFieldBits};
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};

pub mod circuit;
pub mod pcs;
pub mod snark;
pub mod transcript;

use pcs::PCSEngineTrait;
use transcript::{TranscriptEngineTrait, TranscriptReprTrait};

pub use crate::big_num::{DelayedReduction, FieldReductionConstants, MontgomeryLimbs};

/// Represents an element of a group
/// This is currently tailored for an elliptic curve group
pub trait Group: Clone + Copy + Debug + Send + Sync + Sized + Eq + PartialEq {
    /// A type representing an element of the base field of the group
    type Base: PrimeFieldBits + Serialize + for<'de> Deserialize<'de>;

    /// A type representing an element of the scalar field of the group
    type Scalar: PrimeFieldBits
        + PrimeFieldExt
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>;

    /// Returns A, B, the order of the group, the size of the base field as big integers
    fn group_params() -> (Self::Base, Self::Base, BigInt, BigInt);
}

/// A collection of engines that are required by the library
pub trait Engine: Clone + Copy + Debug + Send + Sync + Sized + Eq + PartialEq {
    /// A type representing an element of the base field of the group
    type Base: PrimeFieldBits
        + TranscriptReprTrait<Self::GE>
        + Serialize
        + for<'de> Deserialize<'de>;

    /// A type representing an element of the scalar field of the group
    type Scalar: PrimeFieldBits
        + PrimeFieldExt
        + Send
        + Sync
        + TranscriptReprTrait<Self::GE>
        + Serialize
        + for<'de> Deserialize<'de>
        + FieldReductionConstants
        + DelayedReduction<Self::Scalar>
        + MontgomeryLimbs;

    /// A type that represents an element of the group
    type GE: Group<Base = Self::Base, Scalar = Self::Scalar> + Serialize + for<'de> Deserialize<'de>;

    /// A type that provides a generic Fiat-Shamir transcript to be used when externalizing proofs
    type TE: TranscriptEngineTrait<Self>;

    /// A type that defines a commitment engine over scalars in the group
    type PCS: PCSEngineTrait<Self>;
}

/// Defines additional methods on `PrimeField` objects
pub trait PrimeFieldExt: PrimeField {
    /// Returns a scalar representing the bytes
    fn from_uniform(bytes: &[u8]) -> Self;
}
