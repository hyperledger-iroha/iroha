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
//! - [`export`]: the export of a vendored circuit into the `iroha_plonk` IR:
//!   the configure-time constraint system replayed natively (query tables in
//!   vendored order), the fixed, selector, advice and instance tables and the
//!   copy constraints captured through the vendored `Assignment` trait, native
//!   key generation from them, a node-for-node comparison with the vendored
//!   compressed constraint system, and the vendored `transcript_repr`.
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
//! Milestones M1b and M1c (engine and verifier parity), in
//! `tests/vendored_goldens/`: every vendored golden case is exported with
//! [`export`] (once per process), its native verifying key and parameters
//! must equal the vendored bytes at 1, 2, 4 and 7 threads, and every
//! `DEV-xx` row of spec section 14 is linked to a named test. In oracle
//! builds (`--cfg iroha_plonk_oracle`, a manual run today; TODO: an oracle CI
//! job) the native prover with the injected vendored `transcript_repr` must
//! reproduce every golden SHA-256 at 1, 2, 4 and 7 threads on the Blake2b
//! path, and the vendored KAGEMUSHA path's bytes (Poseidon transcript,
//! folded-generator suffix) on the same circuits; the native verifier must
//! return the vendored verdict on every golden and on the tamper corpora of
//! both paths, except the registered stricter rejections.
//!
//! Not yet covered (TODO(T16), `iroha_core_zk` owner): the KAGEMUSHA goldens
//! of `iroha_core_zk` itself (`sigma_native_k11`, `p256_k16`, `rec_*`), whose
//! circuits live in that crate's private test module.

pub mod convert;
pub mod export;
pub mod pools;
pub mod vendored;
