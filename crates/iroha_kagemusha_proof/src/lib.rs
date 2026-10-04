//! Native-only KAGEMUSHA relations on the PIPA-v1 engine
//! ([`iroha_plonk`], `specs/plonk_ipa_v1.md`).
//!
//! This crate is the compilation boundary of KAGEMUSHA relations built on the
//! native stack: it links [`iroha_pasta`], [`iroha_plonk`] and
//! [`iroha_plonk_gadgets`] only, never the vendored halo2 stack or
//! `iroha_core_zk`.
//!
//! # Prototype status
//!
//! Everything here is a **prototype** of the proposed *split-lineage* design
//! (step proofs on the payment path, the recursive lineage proof in the
//! background), whose owner approval is pending. The relations reproduce the
//! M7 measurement semantics (`g3_proof_scaling_measurement_tests.rs`
//! `m7_step`) with its domain labels (`m7score1`, `m7stmnt1`, ...); they are
//! not a protocol format and no protocol path uses them. A frozen relation
//! will get versioned types, shared Swift/Kotlin vectors and owner sign-off.
//!
//! # Contents
//!
//! - [`witness`]: the native witness types ([`StepWitness`]), the reference
//!   evaluation ([`StepWitness::evaluate`]: every digest, the successor and
//!   the relation [`Violation`]s) and the public outputs ([`StepPublic`]).
//! - [`circuit`]: [`SigmaCircuit`], the step relations `sigma_send` and
//!   `sigma_recv` on Pow5 sponge lanes, running-sum range checks, checked
//!   `u128`/`u64` arithmetic and glue gates, with the two-level (M7
//!   recommendation) or flat state layout and folded or absorbed Poseidon
//!   prefixes.
//! - [`shape`]: [`SigmaShape`] and the shape selector [`select_shape`] (the
//!   smallest `k` that fits, optionally within a proof byte budget), with
//!   exact proof lengths from the descriptor.
//! - [`proof`]: key generation, proving and verification
//!   ([`SigmaProver`], [`SigmaVerifier`]).
//! - [`vectors`]: deterministic sample witnesses with the M7 distributions
//!   and mutations.
//!
//! # Relation (M7)
//!
//! Both steps open the predecessor state commitment (lifecycle Active),
//! check a nonzero `u128` amount and `sequence + 1 < 2^128`, append one
//! chain accumulator and commit the successor. `sigma_send` debits
//! `amount + fee` without overdraft, advances the send ordinal, requires
//! `request_policy_epoch <= policy_epoch` and
//! `max(accepted_time_floor, request_time) <= lower <= upper`, and binds the
//! 24-field Request body; `sigma_recv` credits the amount without overflow.
//! The public outputs are the Poseidon digest of the 32-field G1 statement
//! encoding and, for `sigma_send`, the Request digest.
//!
//! # Determinism
//!
//! Layouts, keys and digests are pure functions of their inputs: ordered
//! containers, checked row arithmetic, no environment variables and no
//! `unsafe`. Proofs depend only on the inputs and the caller's
//! [`iroha_plonk::ProverRandomness`]; the Rayon pool size changes no byte.
#![forbid(unsafe_code)]

pub mod circuit;
pub mod proof;
mod relation;
pub mod shape;
pub mod vectors;
pub mod witness;

pub use circuit::{
    HashSite, Inventory, LanePlan, MAX_LANES, ParamsError, PrefixMode, RelationOutput,
    RelationShape, SigmaCircuit, SigmaConfig, SigmaParams,
};
pub use iroha_plonk_gadgets::statement::StepRelation;
pub use proof::{KeyOptions, SigmaError, SigmaProof, SigmaProver, SigmaVerifier};
pub use shape::{
    PROOF_BYTES_GATE, ProofFormat, ShapeChoice, ShapePolicy, SigmaShape, limb_bits_for,
    select_shape,
};
pub use vectors::{Mutation, sample_witness};
pub use witness::{
    NativeStep, StateLayout, StepDigests, StepInputs, StepPublic, StepWitness, Violation,
};
