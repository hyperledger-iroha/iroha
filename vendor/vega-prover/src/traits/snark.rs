// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module defines a collection of traits that define the behavior of a zkSNARK for RelaxedR1CS
use crate::{
    errors::VegaError,
    traits::{Engine, Group, TranscriptReprTrait, circuit::VegaCircuit},
};
use serde::{Deserialize, Serialize};

/// A trait that defines the behavior of a zkSNARK
pub trait R1CSSNARKTrait<E: Engine>:
    Sized + Send + Sync + Serialize + for<'de> Deserialize<'de>
{
    /// A type that represents the prover's key
    type ProverKey: Send + Sync + Serialize + for<'de> Deserialize<'de>;

    /// A type that represents the verifier's key
    type VerifierKey: Send + Sync + Serialize + for<'de> Deserialize<'de>;

    /// A type that holds the prep work for producing the SNARK
    type PrepSNARK: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>;

    /// Produces the keys for the prover and the verifier
    fn setup<C: VegaCircuit<E>>(
        circuit: C,
    ) -> Result<(Self::ProverKey, Self::VerifierKey), VegaError>;

    /// Prepares the SNARK for proving, given a prover key and a circuit
    fn prep_prove<C: VegaCircuit<E>>(
        pk: &Self::ProverKey,
        circuit: C,
        is_small: bool, // do witness elements fit in machine words?
    ) -> Result<Self::PrepSNARK, VegaError>;

    /// Produces witness and instance for a given circuit, and proves it.
    /// Takes ownership of the prep state and returns it alongside the proof
    /// so it can be rerandomized and reused for subsequent proofs.
    fn prove<C: VegaCircuit<E>>(
        pk: &Self::ProverKey,
        circuit: C,
        prep_snark: Self::PrepSNARK,
        is_small: bool, // do witness elements fit in machine words?
    ) -> Result<(Self, Self::PrepSNARK), VegaError>;

    /// Verifies a SNARK for a relaxed R1CS and returns the public IO
    fn verify(&self, vk: &Self::VerifierKey) -> Result<Vec<E::Scalar>, VegaError>;
}

/// A type representing the digest of a verifier's key
pub type VegaDigest = [u8; 32];

/// A helper trait that defines the behavior of a verifier key of `zkSNARK`
pub trait DigestHelperTrait<E: Engine> {
    /// Returns the digest of the verifier's key
    fn digest(&self) -> Result<VegaDigest, VegaError>;
}

// implement TranscriptReprTrait for the VegaDigest
impl<G: Group> TranscriptReprTrait<G> for VegaDigest {
    fn to_transcript_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }
}
