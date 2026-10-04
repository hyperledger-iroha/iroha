//! Iroha-native PIPA-v1 PLONKish/IPA proof system (`specs/plonk_ipa_v1.md`).
//!
//! `iroha_plonk` reimplements the halo2-axiom arithmetization natively on
//! [`iroha_pasta`]. The vendored halo2 stack is only a test oracle
//! (`crates/iroha_plonk_oracle`); it is never a dependency of this crate.
//!
//! # Contents
//!
//! Stage ENGINE-1 (tasks T8, T9):
//!
//! - [`cs`]: the constraint-system IR. [`cs::Expression`] trees over fixed,
//!   advice and instance queries, gates, halo2 permuted lookups, the chunked
//!   permutation argument and its copy assembly, an exact port of halo2
//!   selector compression, and [`cs::CircuitDescriptorV1`], the canonical
//!   Norito statement of everything a verifier evaluates, with its digest.
//! - [`frontend`]: the circuit API. [`frontend::Circuit`], [`frontend::Layouter`],
//!   [`frontend::Region`], [`frontend::Value`] and [`frontend::Assigned`], a
//!   floor planner whose layout equals the halo2-axiom `SimpleFloorPlanner`, and
//!   the [`frontend::Assembly`] that records fixed values, selectors, copies and
//!   witnesses.
//! - [`check`]: the constraint checker, a naive interpreter over the
//!   uncompressed source expressions with cell-level diagnostics. It replaces
//!   halo2's `MockProver` and is stricter: queries of unassigned advice cells,
//!   of blinding rows and across the domain boundary are reported.
//!
//! Stage ENGINE-2 (tasks T10, T11, T13 PCS):
//!
//! - [`keys`]: the [`keys::VerifyingKey`] `0x02` codec with a strict reader
//!   bound to a [`keys::DescriptorBinding`], the [`keys::ProvingKey`] with the
//!   exact quotient cosets and its fixed-coset cache, and deterministic key
//!   generation with halo2's permutation cycles.
//! - [`transcript`]: the `BLAKE2b` `Challenge255` transcript and the KAGEMUSHA
//!   RP57 Poseidon transcript (injective point absorption in production), the
//!   canonical proof-message decoding and the instance-frame prelude.
//! - [`pcs`]: commitments, the BGH19 IPA whose prover returns the folded
//!   generator, succinct verification into a `#[must_use]` pending
//!   accumulator, `decide` and the deterministic `batch_decide`, and the
//!   multiopen with static query grouping.
//!
//! Stage ENGINE-3 (tasks T12, T13):
//!
//! - [`protocol`]: the tables the prover and verifier share, derived from the
//!   descriptor alone: the shape, the opening queries of spec 9.1 and their
//!   static plan, the exact proof length, the Lagrange and Direct-instance
//!   evaluations, the explicit-stack expression evaluator and the S7
//!   zero-knowledge budget computed from the opening plan.
//! - [`prover`]: [`prover::create_proof`] with the `BlindingScheduleV1` draw
//!   order from an opaque [`prover::ProverRandomness`] (OS, hedged, or a
//!   recovery stream bound to the witness digest), the advice, lookup,
//!   permutation and vanishing commitments, the quotient on exactly `d - 1`
//!   cosets evaluated by a compiled, hash-consed expression DAG, and the
//!   multiopen; [`prover::Witness`] checks a circuit against its key.
//! - [`verifier`]: [`verifier::verify_full`], [`verifier::verify_succinct`]
//!   and [`verifier::batch_verify`] with typed [`verifier::VerifyError`]
//!   rejections: canonical decoding, the exact proof length, exact instance
//!   shapes in both modes, pinned parameters, the descriptor-bound
//!   `transcript_repr`, degenerate-challenge rejection and static opening
//!   groups.
//!
//! In oracle mode the prover reproduces vendored halo2-axiom proof bytes
//! (Committed and Direct instances, both curves, with and without selector
//! compression).
//!
//! # Determinism
//!
//! Every output is a pure function of its inputs: exact field and group
//! arithmetic, ordered maps only, no behaviour sourced from environment
//! variables. Kernels run on the caller's Rayon pool and their results do not
//! depend on the thread count. Prover randomness comes only from the
//! caller's RNG, drawn in the `BlindingScheduleV1` order. Arithmetic on sizes
//! is checked; an overflow is an error, never a wrap.
//!
//! # Soundness rules enforced here
//!
//! - Queries are interned per `(column, rotation)` exactly as halo2 interns
//!   them, and a descriptor with a repeated `(column, rotation)` query, or with
//!   rotations that collide modulo `n`, is rejected (spec section 12, S2).
//! - Opening queries are grouped by slot (column kind and index), never by
//!   commitment value, and a repeated query must repeat its evaluation bit
//!   for bit (S1, S3).
//! - Descriptors fix the exact length of every instance column, and the
//!   transcript prelude frames the instance shape (S4).
//! - Verifiers accept only parameters derived here or matching the pinned
//!   digest per `(curve, k)` (S5), and descriptors pin the zero-knowledge
//!   query budget (S7).
//! - Every proof message decodes canonically, the identity point is never
//!   absorbed, the proof length is exact (trailing bytes are rejected),
//!   `x = 0` and `x^n = 1` are rejected, and only `verify_full`,
//!   `batch_verify`, `decide` and `batch_decide` accept (S9, spec section
//!   11).
//! - Provers draw randomness only from [`prover::ProverRandomness`]; fixed
//!   seeds exist only in unit tests and oracle builds (S8).
//!
//! # Oracle mode
//!
//! The vendored `transcript_repr` injection and the `fe_to_fe` Poseidon point
//! absorption exist only with `--cfg iroha_plonk_oracle` (passed through
//! `RUSTFLAGS` by the oracle CI job) or in this crate's unit tests; they are
//! never a Cargo feature.
#![forbid(unsafe_code)]

pub mod check;
pub mod cs;
pub mod frontend;
pub mod keys;
pub mod pcs;
pub mod protocol;
pub mod prover;
#[cfg(test)]
mod test_circuits;
pub mod transcript;
pub mod verifier;

pub use check::{CheckFailure, CheckMode, CheckReport};
pub use cs::{CircuitDescriptorV1, ConstraintSystem, CsError, DescriptorError, Expression};
pub use frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value};
pub use keys::{DescriptorBinding, KeyError, ProvingKey, VerifyingKey};
pub use pcs::{
    ipa::{
        IpaError, PinnedParams,
        accumulator::{AccumulatorError, PendingAccumulator, batch_decide},
    },
    multiopen::{MultiopenError, OpeningPlan, OpeningQuery, Slot, SlotKind},
};
pub use protocol::{Protocol, ProtocolError, Shape};
pub use prover::{
    ProverConfig, ProverError, ProverRandomness, Witness, create_proof, prove_circuit,
};
pub use transcript::{Transcript, TranscriptError, TranscriptRead, TranscriptWrite};
pub use verifier::{
    BatchItem, VerifyError, batch_verify, verify_full, verify_full_from_bytes, verify_succinct,
};
