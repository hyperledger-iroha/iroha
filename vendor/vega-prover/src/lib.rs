// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This library implements the ZK provers of Vega, optimized for low-latency
//! client-side proving of statements over signed data.
#![deny(
    warnings,
    unused,
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    missing_docs
)]
#![allow(non_snake_case)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![deny(unsafe_code)]

// private modules
mod digest;
mod math;
mod nifs;
mod r1cs;
mod zk;

#[macro_use]
mod macros;

// public modules
pub mod bellpepper;
pub mod errors;
#[doc(hidden)]
pub mod iroha_rng;
pub mod provider;
pub mod traits;

// internal modules
mod big_num;
mod polys;
mod sumcheck;

// public modules for proof systems
pub(crate) mod spartan_relaxed; // single-circuit prover for relaxed R1CS (non-ZK)
pub mod vega_mc_zkp; // multi-circuit (NeutronNova folding) prover with zero-knowledge
pub mod vega_sc; // single-circuit (Spartan) prover without zero-knowledge
pub mod vega_sc_zkp; // single-circuit (Spartan) prover with zero-knowledge

/// Start a span + timer, return `(Span, Instant)`.
macro_rules! start_span {
    ($name:expr $(, $($fmt:tt)+)?) => {{
        let span       = tracing::info_span!($name $(, $($fmt)+)?);
        let span_clone = span.clone();    // lives as long as the guard
        let _guard      = span_clone.enter();
        (span, std::time::Instant::now())
    }};
}
pub(crate) use start_span;

// The default width used for monolithic commitments.
pub(crate) const DEFAULT_COMMITMENT_WIDTH: usize = 2048;

use traits::{Engine, pcs::PCSEngineTrait};
type CommitmentKey<E> = <<E as traits::Engine>::PCS as PCSEngineTrait<E>>::CommitmentKey;
type VerifierKey<E> = <<E as traits::Engine>::PCS as PCSEngineTrait<E>>::VerifierKey;
type Commitment<E> = <<E as Engine>::PCS as PCSEngineTrait<E>>::Commitment;
type PCS<E> = <E as Engine>::PCS;
type Blind<E> = <<E as Engine>::PCS as PCSEngineTrait<E>>::Blind;
