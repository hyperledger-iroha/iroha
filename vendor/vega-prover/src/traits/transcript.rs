// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module provides the trait definitions for transcript functionality in the vega-prover library.
//! Transcripts are used for Fiat-Shamir transformations to make interactive proof systems non-interactive.
use crate::{
    errors::VegaError,
    traits::{Engine, Group},
};

/// This trait allows types to implement how they want to be added to `TranscriptEngine`
pub trait TranscriptReprTrait<G: Group>: Send + Sync {
    /// returns a byte representation of self to be added to the transcript
    fn to_transcript_bytes(&self) -> Vec<u8>;
}

/// This trait defines the behavior of a transcript engine compatible with Spartan
pub trait TranscriptEngineTrait<E: Engine>: Send + Sync {
    /// initializes the transcript
    fn new(label: &'static [u8]) -> Self;

    /// returns a scalar element of the group as a challenge
    fn squeeze(&mut self, label: &'static [u8]) -> Result<E::Scalar, VegaError>;

    /// absorbs any type that implements `TranscriptReprTrait` under a label
    fn absorb<T: TranscriptReprTrait<E::GE>>(&mut self, label: &'static [u8], o: &T);

    /// adds a domain separator
    fn dom_sep(&mut self, bytes: &'static [u8]);
}

impl<G: Group, T: TranscriptReprTrait<G>> TranscriptReprTrait<G> for &[T] {
    fn to_transcript_bytes(&self) -> Vec<u8> {
        self.iter()
            .flat_map(|t| t.to_transcript_bytes())
            .collect::<Vec<u8>>()
    }
}
