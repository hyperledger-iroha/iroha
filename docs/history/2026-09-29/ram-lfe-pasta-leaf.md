# Pinned Pasta native leaf candidate

Date: 2026-09-29. The ordinary `iroha_zkp_poseidon` native suite passes 18 tests,
with zero ignored tests. This qualifies the recorded native candidate only.
No production caller, RAM-LFE hash contract, admitted circuit, key, root or
proof-mode admission changed. The complete semantic design remains proposed in
[the design record](../../../specs/ram_lfe_semantic_commitments.md).

The leaf's new `pasta` module implements the exact pinned upstream P128Pow5T3
permutation over Fp and Fq and positive-length `ConstantLength<L>` hashing.
Its byte API rejects noncanonical field values without reduction; permutation
input remains unchanged on a late decoding error. State, matrix and byte
representation scratch use clearing owners. Caller buffers, outputs, arithmetic
temporaries and compiler copies remain outside the clearing guarantee.

Seven new controls cover all 44 upstream public vectors (11 permutations and
11 fixed-length hashes per field), independent arithmetic with Core's actual
`halo2curves-axiom::pasta` field types, parameter/vector SHA-256 fingerprints,
zero/p - 1 acceptance, p/p + 1/all-ones rejection, late error preservation,
odd/even and trailing-zero separation, lengths through 2,054, and actual owned
field-cell clearing on success/error/unwind. Eleven existing leaf controls
also pass. Public parameter/vector files and upstream provenance are recorded
beside the source in `crates/iroha_zkp_poseidon/src/pasta/README.md`.

The sole production dependency addition is the already-resolved `zeroize`.
`halo2curves-axiom` and `sha2` are test-only dependencies. Cargo.lock changes
only the three corresponding entries in this leaf's dependency list; it adds
no package version or resolution change.

## Immutable ordinary native evidence

The runner uses the ordinary workspace package and its unmodified test profile:

```text
cargo +1.93.1 test --locked --offline -p iroha_zkp_poseidon --lib --no-run --message-format=json -j 2
```

`CARGO_TARGET_DIR` is `target/zk-pasta-leaf`. It then copies the resulting test
binary and executes that immutable copy with two test threads. The final record
is `dist/zk-remediation/2026-09-29/ram-lfe-pasta-leaf/20260929T064156Z`.
All 158 captured local sources, including patched Halo2 curves and num-bigint,
match before/after execution. The retained binary also matches after execution.

- Result: 18 passed, 0 failed, 0 ignored; native execution 1.68 s.
- Warm build plus native execution: 10.592 s.
- Peak child-process RSS: 662,863,872 B.
- Binary SHA-256: `74001682efbb31acdc1762a777f6966a8e5e6dd1d4e5ff56df7900777378ae5c`.
- `pasta.rs`: `643defb895437930c961ea94203f5f190f0fc90a5bfb29dfee7f4808353da8fb`.
- `pasta/tests.rs`: `badae3316d2fbc09370ee9be88e9f8bee0a111e17370636aae5ff7cdc768aad7`.

Strict package Clippy (`--all-targets -- -D warnings`), scoped rustfmt and
`git diff --check` pass on this source. This record is native primitive
qualification, not a Halo2 proof, full Core package build or security review.

## Retained predecessor outcomes

- `20260929T063638Z` failed manifest loading before compilation: two test
  dependencies were incorrectly declared as inherited workspace dependencies.
  The repair uses their already-resolved explicit versions and root patch.
- `20260929T063658Z` failed compilation because halo2curves 0.9 represents
  fields with `Repr<32>`, while Core's Pasta implementation uses `[u8; 32]`.
  The repair uses strict byte adapters and clearing representation scratch.
- `20260929T063938Z` passes all 18 native tests. Its narrower source inventory
  did not include patched num-bigint. A subsequent strict Clippy run rejected
  one test-only manual range expression. The repair uses explicit full-round
  pattern matching, and the final capture includes num-bigint. No permutation,
  sponge or input-bound constraint changed in that repair.

The semantic design now separates stable keyed logical-function identity from
encryption/relinearization policy binding. Initializer derivation excludes
rotatable encryption keys. An opaque identifier still requires authenticated
opened plaintext and the separate trusted opening authority. The proposed
global Pasta parameter cut, canonical profile naming, native/circuit parity,
maximum complete proof resources and independent cryptographic review remain
unimplemented or unqualified.
