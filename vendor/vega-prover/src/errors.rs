// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module defines errors returned by the library.
use core::fmt::Debug;
use thiserror::Error;

/// Errors returned by vega-prover
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum VegaError {
    /// returned if the supplied row or col in (row,col,val) tuple is out of range
    #[error("InvalidIndex")]
    InvalidIndex,
    /// returned if the step circuit calls inputize or alloc_io in its synthesize method
    /// instead of passing output with the return value
    #[error("InvalidStepCircuitIO")]
    InvalidStepCircuitIO,
    /// returned if the supplied input is not of the right length
    #[error("InvalidInputLength")]
    InvalidInputLength {
        /// The reason for the invalid input check failure
        reason: String,
    },
    /// returned if the supplied witness is not of the right length
    #[error("InvalidWitnessLength")]
    InvalidWitnessLength,
    /// returned if the supplied witness is not a satisfying witness to a given shape and instance
    #[error("UnSat: {reason}")]
    UnSat {
        /// The reason for circuit UnSat failure
        reason: String,
    },
    /// returned if proof verification fails
    #[error("ProofVerifyError: {reason}")]
    ProofVerifyError {
        /// The reason for the proof verification error
        reason: String,
    },
    /// returned if the provided commitment key is not of sufficient length
    #[error("InvalidCommitmentKeyLength")]
    InvalidCommitmentKeyLength,
    /// returned if the provided commitment is not of sufficient length
    #[error("InvalidCommitmentLength: {reason}")]
    InvalidCommitmentLength {
        /// The reason for the invalid commitment length error
        reason: String,
    },
    /// returned when an invalid PCS evaluation argument is provided
    #[error("InvalidPCS")]
    InvalidPCS {
        /// The reason for the invalid PCS error
        reason: String,
    },
    /// returned when an invalid sum-check proof is provided
    #[error("InvalidSumcheckProof")]
    InvalidSumcheckProof,
    /// returned when the initial input to an incremental computation differs from a previously declared arity
    #[error("InvalidInitialInputLength")]
    InvalidInitialInputLength,
    /// returned when the step execution produces an output whose length differs from a previously declared arity
    #[error("InvalidStepOutputLength")]
    InvalidStepOutputLength,
    /// returned when the transcript engine encounters an overflow of the round number
    #[error("InternalTranscriptError")]
    InternalTranscriptError,
    /// returned when the multiset check fails
    #[error("InvalidMultisetProof")]
    InvalidMultisetProof,
    /// returned when the product proof check fails
    #[error("InvalidProductProof")]
    InvalidProductProof,
    /// returned when the consistency with public IO and assignment used fails
    #[error("IncorrectWitness")]
    IncorrectWitness {
        /// The reason for the incorrect witness error
        reason: String,
    },
    /// return when error during synthesis
    #[error("SynthesisError: {reason}")]
    SynthesisError {
        /// The reason for circuit synthesis failure
        reason: String,
    },
    /// returned when there is an error creating a digest
    #[error("DigestError")]
    DigestError {
        /// The reason for the digest error
        reason: String,
    },
    /// returned when the prover cannot prove the provided statement due to completeness error
    #[error("InternalError")]
    InternalError {
        /// The reason for the internal error
        reason: String,
    },
    /// returned when attempting to divide by zero
    #[error("DivisionByZero")]
    DivisionByZero,
    /// returned when input vector size exceeds commitment key capacity
    #[error("InvalidVectorSize")]
    InvalidVectorSize {
        /// The actual size of the input vector
        actual: usize,
        /// The maximum size that can be handled
        max: usize,
    },
}
