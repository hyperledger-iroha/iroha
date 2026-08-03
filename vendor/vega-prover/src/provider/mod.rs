// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module implements Spartan's traits using the following several different combinations

// public modules to be used as an commitment engine with Spartan
pub mod bn254;
pub mod keccak;
pub mod pasta;
pub mod pcs;
pub mod pt256;
pub mod traits;

mod msm;

use crate::{
    provider::{
        bn254::types as bn254_types,
        keccak::Keccak256Transcript,
        pasta::{pallas, vesta},
        pcs::hyrax_pc::HyraxPCS,
        pt256::{p256, t256},
    },
    traits::Engine,
};
use core::fmt::Debug;
use serde::{Deserialize, Serialize};

/// An implementation of the Spartan Engine trait with Pallas curve and Hyrax commitment scheme
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PallasHyraxEngine;

/// An implementation of the Spartan Engine trait with Vesta curve and Hyrax commitment scheme
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VestaHyraxEngine;

/// An implementation of the Spartan Engine trait with P256 curve and Hyrax commitment scheme
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct P256HyraxEngine;

/// An implementation of the Spartan Engine trait with T256 curve and Hyrax commitment scheme
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct T256HyraxEngine;

/// An implementation of the Spartan Engine trait with BN254 curve and Hyrax commitment scheme
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Bn254Engine;

impl Engine for PallasHyraxEngine {
    type Base = pallas::Base;
    type Scalar = pallas::Scalar;
    type GE = pallas::Point;
    type TE = Keccak256Transcript<Self>;
    type PCS = HyraxPCS<Self>;
}

impl Engine for VestaHyraxEngine {
    type Base = vesta::Base;
    type Scalar = vesta::Scalar;
    type GE = vesta::Point;
    type TE = Keccak256Transcript<Self>;
    type PCS = HyraxPCS<Self>;
}

impl Engine for P256HyraxEngine {
    type Base = p256::Base;
    type Scalar = p256::Scalar;
    type GE = p256::Point;
    type TE = Keccak256Transcript<Self>;
    type PCS = HyraxPCS<Self>;
}

impl Engine for T256HyraxEngine {
    type Base = t256::Base;
    type Scalar = t256::Scalar;
    type GE = t256::Point;
    type TE = Keccak256Transcript<Self>;
    type PCS = HyraxPCS<Self>;
}

impl Engine for Bn254Engine {
    type Base = bn254_types::Base;
    type Scalar = bn254_types::Scalar;
    type GE = bn254_types::Point;
    type TE = Keccak256Transcript<Self>;
    type PCS = HyraxPCS<Self>;
}
