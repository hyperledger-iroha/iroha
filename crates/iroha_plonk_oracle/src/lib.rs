//! Temporary differential test oracle for the Iroha-native PLONK/IPA stack.
//!
//! This is the only crate that links both the vendored halo2 stack
//! (`halo2-axiom`, `halo2-base`, `snark-verifier`) and the native crates such as
//! [`iroha_pasta`]. It exists to prove that the native implementation matches
//! the vendored one byte for byte:
//!
//! - export vendored constraint systems (compressed selectors, fixed, advice and
//!   instance tables, copy constraints, `transcript_repr`) into the native IR;
//! - run the vendored golden proofs and re-prove every case natively;
//! - compare params, verifying-key and proof bytes, and verifier verdicts on
//!   tamper corpora.
//!
//! The crate is `publish = false` and is used only as a dev-dependency. No
//! shipping crate may depend on it. It is deleted together with the vendored
//! halo2 stack once every consumer has migrated.
//!
//! # Library modules
//!
//! - [`convert`]: canonical conversions between the vendored Pasta types and
//!   the `iroha_pasta` types, per half of the cycle ([`convert::Vesta`],
//!   [`convert::Pallas`]).
//! - [`pools`]: shared Rayon pools of 1, 2, 4 and 7 threads for
//!   thread-count independence checks.
//! - [`vendored`]: oracle access to crate-private vendored behaviour (the IPA
//!   generator fold) and a recording transcript wrapper.
//!
//! # Tests
//!
//! Milestone M0 (contract capture):
//!
//! - `tests/vendored_goldens.rs` runs the vendored golden proofs unchanged at 1,
//!   2, 4 and 7 threads;
//! - `tests/native_prover_kats.rs` generates and checks
//!   `fixtures/native_prover/kats_v1.json` (params, generators, transcripts,
//!   Poseidon constants and native hash vectors).
//!
//! Milestone M1a (`iroha_pasta` parity), `tests/pasta_parity/`: byte parity of
//! `iroha_pasta` with the vendored stack at 1, 2, 4 and 7 threads. It covers
//! field and curve encodings against the `halo2curves-axiom` Pasta types;
//! `ParamsIPA` bytes; MSM against `best_multiexp`; the generator fold against
//! the vendored IPA collapse and real vendored IPA proofs; FFT, IFFT and coset
//! transforms against `EvaluationDomain` and the vendored FFT backends; and the
//! RP57 Poseidon sponge against `snark-verifier` and the KAT fixture.
//!
//! `tests/kernel_benchmarks.rs` holds ignored release microbenchmarks that time
//! the native kernels against the vendored ones side by side.
//!
//! TODO: the constraint-system export and the prover, verifying-key and
//! verifier differential suites land with milestones M1b and M1c.

pub mod convert;
pub mod pools;
pub mod vendored;
