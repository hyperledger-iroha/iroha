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
//! background; `specs/kagemusha_single_design_proposal.md` sections 3 and
//! 3.2), whose owner approval is pending. The relations implement the spec
//! section 3 core with every control off, under prototype domain labels
//! (`kgspcor1`, `kgspcrd1`, ...); only the public statement follows the G1
//! wallet statement encoding (`kgwstmt1`). They are not a protocol format
//! and no protocol path uses them. A frozen relation will get versioned types,
//! shared Swift/Kotlin vectors and owner sign-off.
//!
//! # Contents
//!
//! - [`witness`]: the native witness types ([`StepWitness`]), the reference
//!   evaluation ([`StepWitness::evaluate`]: every digest, the successor and
//!   the relation [`Violation`]s), the Request body ([`RequestBody`]) and
//!   the public outputs ([`StepPublic`]).
//! - [`circuit`]: [`SigmaCircuit`], the step relations `sigma_send` and
//!   `sigma_recv` on Pow5 sponge lanes, running-sum range checks, checked
//!   `u128`/`u64` arithmetic and glue gates, with the two-level (spec) or
//!   flat state layout and folded or absorbed Poseidon prefixes.
//! - [`consumer`]: the native spec section 3.2 checks a package consumer
//!   runs on a statement before verifying its proof.
//! - [`shape`]: [`SigmaShape`] and the shape selector [`select_shape`] (the
//!   smallest `k` that fits, optionally within a proof byte budget), with
//!   exact proof lengths from the descriptor.
//! - [`proof`]: key generation, proving and verification
//!   ([`SigmaProver`], [`SigmaVerifier`]).
//! - [`vectors`]: deterministic sample witnesses and relation mutations.
//!
//! # Relation
//!
//! Both steps open the predecessor state commitment (lifecycle Active),
//! check a nonzero `u128` amount and `sequence + 1 < 2^128`, derive the
//! credit identifier `H(kgspcrd1, Request body)` in circuit (with the
//! identity fields of the Request taken from the opened core), require
//! distinct payer and receiver wallets, append one chain accumulator and
//! commit the successor. `sigma_send` takes `burned_total` and the
//! pending-outgoing root of the predecessor's lineage proof as public
//! inputs, requires an empty enabled-controls mask and
//! `amount + fee <= balance - burned_total` (checked), advances the send
//! ordinal, requires `request_policy_epoch <= policy_epoch` and
//! `max(accepted_time_floor, request_time) <= lower <= upper`, and raises the
//! successor's accepted-time floor to `lower`; `sigma_recv` credits the
//! amount without overflow. The public outputs are the Poseidon digest of
//! the 29-field G1 step statement and, for `sigma_send`, the credit
//! identifier.
//!
//! # Determinism
//!
//! Layouts, keys and digests are pure functions of their inputs: ordered
//! containers, checked row arithmetic, no environment variables and no
//! `unsafe`. Proofs depend only on the inputs and the caller's
//! [`iroha_plonk::ProverRandomness`]; the Rayon pool size changes no key or
//! proof byte (`tests/real_proofs.rs` checks 1, 2, 4 and 7 threads).
#![forbid(unsafe_code)]

pub mod circuit;
pub mod consumer;
pub mod proof;
mod relation;
pub mod shape;
pub mod vectors;
pub mod witness;

pub use circuit::{
    HashSite, Inventory, LanePlan, MAX_LANES, ParamsError, PrefixMode, RelationOutput,
    RelationShape, SigmaCircuit, SigmaConfig, SigmaParams,
};
pub use consumer::{ConsumerError, LineageView, check_receive, check_send};
pub use iroha_plonk_gadgets::statement::{StatementV1, StepRelation};
pub use proof::{KeyOptions, SigmaError, SigmaProof, SigmaProver, SigmaVerifier};
pub use shape::{
    PROOF_BYTES_GATE, ProofFormat, ShapeChoice, ShapePolicy, SigmaShape, limb_bits_for,
    select_shape,
};
pub use vectors::{Mutation, sample_witness};
pub use witness::{
    LineageInputs, NativeStep, RequestBody, RequestTerms, StateLayout, StepDigests, StepInputs,
    StepPublic, StepWitness, Violation, relation_id,
};
