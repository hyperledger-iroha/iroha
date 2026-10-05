//! Deterministic exact arithmetic shared by Iroha's homomorphic-encryption protocols.
//!
//! This crate is the single owner of the reusable ring arithmetic below the
//! protocol layers: BFV and RAM-LFE in `iroha_crypto`, the ZK-AMS multi-key
//! BGV profile in `iroha_zkp_halo2`, and the Jindo and Bootle-Lantern rings in
//! `iroha_core_privacy`. It holds arithmetic only. Schemes, parameter sets,
//! key and ciphertext formats, wire codecs, noise accounting and proofs stay
//! with their protocols, which bind their parameters to these kernels and map
//! the typed errors to their own diagnostics.
//!
//! # Modules
//!
//! - [`modular`]: scalar arithmetic modulo a word modulus, primality and
//!   primitive-root search.
//! - [`constant_time`]: branch-free selects and fixed-modulus Montgomery
//!   arithmetic for secret-dependent polynomials.
//! - [`ntt`]: radix-2 cyclic transforms, negacyclic twisting, negacyclic
//!   products and the exact CRT convolution.
//! - [`polynomial`]: coefficient-wise arithmetic and the schoolbook negacyclic
//!   products that serve as fallback and reference.
//! - [`rns`]: modulus-chain validation, residue decomposition, exact CRT
//!   reconstruction and basis extension.
//! - [`rounding`]: centered lifting, nearest division, scale-and-round and
//!   modulus-switch rounding, with every rule stated.
//! - [`key_switch`]: digit decomposition and the digit/key inner product.
//! - [`automorphism`]: the signed coefficient permutation of `X -> X^k`.
//! - [`accel`]: accelerated slice kernels, their dispatch and scalar references.
//!
//! # Determinism
//!
//! Every function computes an exact integer result, so the same inputs give
//! the same words on every target. The scalar implementation is the semantic
//! reference. The `simd` feature adds NEON and AVX2 kernels that are tested
//! word-for-word against it and fall back to it for any input they do not
//! support. It is a default feature of this crate; workspace consumers disable
//! default features and enable it through `iroha_crypto/bfv-accel`. No
//! environment variable, configuration value or wall-clock measurement
//! influences a result.
//!
//! The NEON kernel is tested natively on `AArch64`. The AVX2 kernel is tested
//! with the CPU's instructions on x86-64 hosts that have AVX2, and on every
//! other host through a lane model of its intrinsics (see [`accel`]).
//!
//! Metal and CUDA kernels for this arithmetic do not exist yet: the
//! repository's GPU transforms are specialised to the Goldilocks proof field
//! and live above this crate. The dispatch site in [`ntt::cyclic_ntt_in_place`]
//! carries the `TODO`.
//!
//! # Secrets
//!
//! Kernels hold temporary copies of operands and partially built results in
//! clearing buffers, which are cleared on return, on a rejected input and when
//! a kernel unwinds; `tests/clearing.rs` observes the released memory on each
//! of those exits. Values a kernel returns belong to the caller, which clears
//! them when they are secret. [`constant_time`] is branch-free in operand values; the
//! word-modulus kernels of [`modular`] use `u128` remainders for products and
//! are not constant-time.
pub mod accel;
pub mod automorphism;
pub mod constant_time;
pub mod key_switch;
pub mod modular;
pub mod ntt;
pub mod polynomial;
pub mod rns;
pub mod rounding;
