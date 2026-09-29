# RAM-LFE constrained word and BLAKE3 compression prototype

Date: 2026-09-29. Status: internal test-only primitive; complete execution proof
remains unavailable. No relation identifier or production admission was added.

## Scope and constraints

Core's `zk/ram_lfe_word.rs` implements exact 32-/64-bit load, XOR, modular
three-word addition and rotation over typed nibble cells. Its six advice columns
query only the current row. One 3,905-row finite lookup table binds integer
operands and results. Addition uses a polynomial carry equation over copied,
already-proven input nibbles; each output nibble and carry is range constrained.
Whole-nibble rotation uses copy permutations. Other rotations fuse XOR and the
integer split, with cyclic copies binding the adjacent low bits. A coordinated
inverse-two assignment is rejected by lookup constraints even when no downstream
range or public-output constraint consumes the result.

The raw BLAKE3 compression uses seven constrained rounds, the exact message
permutation and all16 feed-forward output words. Tests compare official BLAKE3
1.8.5 vectors for0,3 and64 bytes; changed CV, message, counters, length, flags and
arbitrary internal field assignments fail. This is compression arithmetic only:
full hash framing, padding, chunk/tree/XOF roles, private Norito CRC64, Blake2b,
BFV arithmetic, machine transitions and receipt bindings remain incomplete.

Host word values have clearing owners, observed zero on success/error/unwind.
This does not claim clearing Halo2's own private assignment/prover buffers or
compiler-created copies. Current proof admission remains unavailable.

## Native evidence

The standalone, untracked harness copies exact Core word/test/adapter files and
actual vendored Halo2 sources. It invokes ordinary offline Cargo with Rust1.93.1,
two jobs, optimized test profile and an isolated target; it copies the immutable
test binary before executing the full discovered module inventory. Before/after
source guards found no drift. No new workspace crate or dependency was added.
This validates the copied primitive and adapter, not a full Core package build.

| Artifact suffix | Controls | Table rows | k | Proof bytes | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| `word-20260929T052907Z` | 7 | 12,785 | 14 | 3,008 | PASS |
| `word-20260929T053444Z` | 8 | 3,905 | 12 | 2,816 | PASS |
| `word-20260929T053900Z` | 11 | 3,905 | 12 | 2,816 | PASS |
| `word-20260929T054420Z` | 12 | 3,905 | 12 | 2,816 | PASS |

All artifacts are retained under
`dist/zk-remediation/2026-09-29/ram-lfe-circuit/`. Every passing run has zero failed
or ignored tests. The initial `word-20260929T052826Z` harness workspace-membership
failure is retained; it occurred before Rust compilation. The harness's explicit
vendor exclusions repaired it without changing production code.

Latest binary SHA-256:
`86bddd5fca82027f77434084220f8ff370a57f916ee023b86f3a402406535e04`.
Latest raw-compression measurements:3,976 operation rows,586B processed VK,
1,214.151ms parameter/key generation,612.333ms proof generation,22.713ms
verification. Whole12-control process peak RSS:43,335,680B. Proof, public-output,
trailing-byte and witness mutations fail. These sample times do not establish the
20ms soft verifier budget or maximum-profile resource limits.

The geometry control measures degree5, six advice queries, one lookup, eight
permutation columns, three columns per permutation product, five blinding factors
and eight minimum rows. Its fixed-column/query counts before selector compression
are both nine.

## Capacity finding and remaining work

For the current layout, a Blake2b G needs128 rows and compression12,544 rows before
loads. Two maximum64-slot ciphertext frames require1,176 compressions, yielding
14,751,744 rows. At k16 this needs at least226 replicated lanes. The transcript
emits at least188,032B for advice commitments/evaluations, permutation-sigma
evaluations and lookup arguments. Degree5 adds at least452 permutation products,
with57,824B of commitments/evaluations, giving a245,856B lower bound. This exceeds
the192KiB default before CRC64/BLAKE3/BFV, fixed evaluations, quotient commitments,
IPA and the outer envelope. The75,187B frame size is independently confirmed by the parent's normal
65-test canonical-transcript suite (binary SHA-256
`32af8c3ad619e217cd9c6c1149c8e246a0a05cb631cc160df840742b23000fd6`);
that evidence is recorded separately.

This is a bound for replication of this specific degree5 layout, not every
possible layout. Further compact hash layout or separately justified composition
is required. `capacity.py/json` retain exact source-derived counts and
`capacity-initial.py/json` preserve their predecessor. No limit was raised.
Full Core compilation, full semantic constraints, maximum-size proof/resource
qualification and independent soundness/clearing review remain open.

## Exact latest primitive sources

- `crates/iroha_core/src/zk/ram_lfe_word.rs`: `86d54a0dfe425e2c82b142f32c816804a96c8e2767d96541ea586010f5eb2034`.
- `crates/iroha_core/src/zk/ram_lfe_word_tests.rs`: `689702d0c360acd050f492e0622592193862ba9a6cba966756b808d8dc9b07e4`.
- `crates/iroha_core/src/zk/halo2_backend.rs`: `b8a49b8b7b2f11500962e2823c8c59b2c9602e4121bdb722498c5e618e74e969`.
