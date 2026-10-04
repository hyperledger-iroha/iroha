//! Workspace runner for the vendored halo2-axiom golden proof bytes.
//!
//! `vendor/halo2-axiom/tests/golden_proof_bytes.rs` has no runner of its own:
//! Cargo refuses `cargo test -p halo2-axiom` for the patched non-member. This
//! target compiles that file unchanged through `#[path]`, against the patched
//! halo2-axiom and halo2curves-axiom that `iroha_core_zk` links, with the same
//! features. Nothing under `vendor/` is edited.
//!
//! Every vendored case proves the sigma-shaped (k = 6 and 9) and wide (k = 8
//! and 10) circuits over both Pasta cycles with prover seeds 42 and 43, inside
//! Rayon pools of 1, 2, 4 and 7 threads. The bytes must not depend on the pool,
//! each proof must verify, and its SHA-256 must equal the vendored constant.
//! The k = 11 sigma cases are ignored release cases.
//!
//! Oracle baseline (recorded with the vectors in
//! `fixtures/native_prover/kats_v1.json`, section `oracle_baseline`): repository
//! HEAD `1de7210a74d62ae5232c67b910dbf4b6b1bcf757`, last `vendor/halo2-axiom`
//! commit `8f41274044c93ad7e363fdc8be30a671efb436d4`. The golden table itself is
//! pinned in the same fixture (section `golden_proofs`), so a vendored constant
//! that changes fails `tests/native_prover_kats.rs` as well.
//!
//! Run:
//! - `cargo test -p iroha_plonk_oracle --test vendored_goldens`
//! - `cargo test --release -p iroha_plonk_oracle --test vendored_goldens -- --include-ignored`

// The vendored file is reviewed upstream code that this crate must not edit:
// `rustfmt::skip` keeps `cargo fmt` from rewriting it through this include, and
// pedantic lints belong to its own crate, not here.
#[allow(clippy::pedantic)]
#[rustfmt::skip]
#[path = "../../../vendor/halo2-axiom/tests/golden_proof_bytes.rs"]
mod golden_proof_bytes;
