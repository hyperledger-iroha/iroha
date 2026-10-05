# iroha_fhe

Deterministic exact arithmetic shared by Iroha's homomorphic-encryption
protocols. This crate is the single owner of the reusable ring arithmetic below
the protocol layers; `specs/fhe_ownership_inventory.json` records which
function owns each primitive and `scripts/check_fhe_ownership_map.py` checks
that record against the source.

It holds arithmetic only. Schemes, parameter sets, key and ciphertext formats,
wire codecs, noise accounting and proofs stay with their protocols:

| Consumer | Location | What it keeps |
| --- | --- | --- |
| BFV and RAM-LFE | `crates/iroha_crypto/src/fhe_bfv.rs` | wire types, parameter policy, guard order, diagnostics, key switching flows |
| ZK-AMS multi-key BGV | `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe*` | profile, wire, collective protocols, test-only references |
| Jindo | `crates/iroha_core_privacy/src/privacy_engines/jindo` | pinned primes and roots, RNS polynomial type |
| Bootle-Lantern | `crates/iroha_core_privacy/src/privacy_engines/bootle_lantern` | pinned Montgomery constants and CRT profile, ring types |

Consumers call the kernels by their own names: a renamed import of a function
or type hides call sites from the ownership inventory, and its tests reject one.

The crate depends on `zeroize` and `thiserror` only, so nothing above it can be
pulled in and the dependency graph stays acyclic.

## Modules

| Module | Content |
| --- | --- |
| `modular` | add, subtract, multiply, power and inverse modulo a word modulus; Miller-Rabin; primitive-root search |
| `constant_time` | branch-free selects, canonical add/subtract, fixed-modulus Montgomery arithmetic, centered three-prime CRT |
| `ntt` | radix-2 cyclic transform, negacyclic twist and product, exact CRT convolution |
| `polynomial` | coefficient-wise arithmetic and the schoolbook negacyclic products, one residue kernel generic over word and fixed-modulus arithmetic |
| `rns` | modulus-chain validation, residue decomposition, CRT reconstruction, basis extension |
| `rounding` | centered lift, nearest division, scale-and-round, modulus-switch rounding |
| `key_switch` | digit decomposition and the digit/key inner product |
| `automorphism` | the signed coefficient permutation of `X -> X^k` |
| `accel` | accelerated slice kernels, their dispatch and scalar references |

## Rounding rules

No floating point is used. Every rule is stated in `rounding` and tested at
exact halves and range boundaries:

- centered lift of `x` modulo `m`: `x` when `x <= floor(m / 2)`, else `x - m`
  (the even midpoint is positive);
- nearest division and scale-and-round: nearest integer, ties away from zero;
- modulus switching: centered lift, scale by `to / from` with the rule above,
  then the least non-negative residue modulo the target.

## Determinism and acceleration

Every function returns an exact integer result, so equal inputs give equal
words on every target. The scalar implementation is the semantic reference.

The `simd` feature adds NEON (AArch64) and AVX2 (x86-64, detected at run time)
kernels for modular addition, subtraction, multiplication and the transform
butterflies. They require canonical operands; sums require a modulus below
`2^62` and products an odd modulus below `2^32`. Any other input takes the
scalar path, and each dispatching function is tested word-for-word against its
`_scalar` reference. No environment variable or configuration value selects a
backend.

`simd` is a default feature of this crate, so its own tests cover the kernels.
Workspace consumers depend on it with `default-features = false`;
`iroha_crypto/bfv-accel` is the one feature that turns the kernels on in a
node build.

The NEON kernel runs natively on every AArch64 host. The AVX2 kernel runs with
the CPU's instructions on x86-64 hosts that have AVX2. Test builds on other
hosts compile the same kernel source over `accel/avx2_model.rs`, a lane model
of the twelve intrinsics it uses, and compare it with the scalar reference; on
x86-64 a test compares each modelled intrinsic with the CPU. The model checks
the kernel's schedule. It is not a run of the instructions.

Metal and CUDA kernels for word-modulus arithmetic do not exist yet. The GPU
transforms in `crates/fastpq_prover` are specialised to the Goldilocks proof
field and sit above this crate. The dispatch site in
`ntt::cyclic_ntt_in_place` carries the `TODO`.

## Secrets

Kernels hold copies of operands and partially built results in clearing
buffers, which are cleared on return, on a rejected input and on unwind.
`tests/clearing.rs` installs an allocator that inspects freed blocks and
asserts this for each of those exits. A value a kernel returns belongs to the
caller. `constant_time` is branch-free in operand values; `modular` is not.

## Tests

```sh
cargo test -p iroha_fhe                         # accelerated kernels and scalar references
cargo test -p iroha_fhe --no-default-features   # scalar-only build
```

`tests/canonical_vectors.rs` pins known-answer vectors for every primitive.
The answers were derived with Python integers, and the test re-derives each one
with an independent `num-bigint` oracle built from the textbook definitions
(quadratic DFT, schoolbook negacyclic product, constructive CRT, exact rational
rounding), so neither the kernels nor the vectors can drift unnoticed.
