# Pinned Pasta circuit experiment

Date: 2026-09-29. Eight native controls pass, with zero ignored tests, in an
isolated harness compiling the exact Core circuit/adapter sources and shared
native leaf. This is an unused test-only candidate, not a normal Core package
build or a RAM execution-proof qualification. Existing admitted 57-round
helpers, keys, roots and production limits remain unchanged. The insecure
diagnostic BFV profile requires replacement independently of this experiment.

`crates/iroha_core/src/zk/ram_lfe_poseidon.rs` implements the pinned upstream
8 + 56 original Poseidon permutation and `ConstantLength<L>` contract over Fp
and Fq. Parameters come only from `iroha_zkp_poseidon::pasta`. Five advice
columns constrain full/partial rounds, length capacity, copied absorption and
odd-length zero padding. No host hash callback supplies the constraints.

The controls cover all 22 upstream two-input hash vectors, odd/even lengths,
384 propagated arbitrary-field round mutations, coordinated initial-state,
absorption, source-input, source-copy and padding mutations, and owned-cell
clearing on success/error/unwind in both fields. Mutation tests omit public
output binding so it cannot conceal a missing local constraint. A genuine Fp
IPA proof rejects a changed public instance, altered proof and trailing bytes.
Fq has native/MockProver parity here; no Fq IPA proof is claimed.

## Recorded measurements

The circuit has degree 6, five advice columns, eight advice queries, no lookups,
three fixed columns before selectors, six permutation columns, five blinding
factors and eight minimum rows. A hash of L fields takes
`65 * ceil(L / 2) + 1` rows; the harness reserves another L disjoint rows for
original input cells. The sample of three fields therefore uses 131 hash rows
plus three source rows at test k=8.

The maximum record of 2,054 fields takes 66,756 hash rows plus 2,054 source rows.
Both fields satisfy the circuit at test k=17. Explicit capacity preflight
rejects k=16 before a domain-sized allocation. This single-lane layout does
not fit the unchanged production k=16 limit. Multi-lane geometry and a complete
semantic/interpreter relation remain unimplemented and unqualified.

The final sample proof is 2,080 bytes, with a 458-byte processed verifying key.
One native measurement gave key generation 83.321 ms, proving 61.019 ms and
verification 3.024 ms. These are sample measurements, not maximum-profile
resource claims. All eight controls ran in 1.51 s; total warm build and test
time was 22.596 s. Test-process peak RSS was 214,859,776 bytes.

Owned working scratch contains eight field elements (256 bytes). Input bytes
use a clearing shared owner, at most 65,728 bytes for this experiment. Tests
observe the actual cells after zeroization. Caller copies, arithmetic/compiler
temporaries and Halo2's witness/prover buffers are outside this guarantee.

## Immutable evidence and limitations

Final evidence is under ignored
`dist/zk-remediation/2026-09-29/ram-lfe-circuit/poseidon-20260929T071041Z`.
The runner captures the Core circuit/tests and actual Halo2 adapter, the shared
leaf, patched dependencies, manifests, lockfile, toolchain and Cargo config.
It uses an isolated Cargo manifest with optimized test/dev profiles, two build
jobs and four Rayon threads; no main workspace profile is changed. Ordinary
Cargo builds the harness, then an immutable copied binary runs its complete
eight-test list. All 349 source-snapshot inputs and 680 captured/actual immutable
compilation paths match before/after execution. Reused dependency sources are
also hash-checked against the new capture. No moving-main build is claimed.

- Binary SHA-256: `58abec8b6e30a7e397791c15c50bb9923927648b39c8bfd427f1e25c6713670e`.
- Circuit source: `75154a57f485d2b061b129f77c6fd4dfd5c4d4490604a8c5892349209a2d9395`.
- Test source: `c9ab5592c32968ea77d5e99a086c76a6ad87d3ae2cdfc9cf4815958c3bd33c14`.
- Shared native leaf: `643defb895437930c961ea94203f5f190f0fc90a5bfb29dfee7f4808353da8fb`.

Scoped rustfmt and whitespace checks pass. Isolated `clippy -- -D clippy::all`
does not pass: the existing copied `halo2_backend.rs:171` uses
`drop(transcript)`, which triggers `drop_non_drop`. That adapter was not
changed or lint-suppressed. Five dead-code warnings arise because the harness
uses only part of that adapter. The new circuit's collapsible-if finding was
fixed and the complete native suite recaptured.

Predecessor records are retained. `065248Z` failed compilation because the
generic test field lacked MockProver's trait bounds. `065746Z` and `070338Z`
passed 6/7: a coordinated input mutation exposed source/absorption cell aliasing
in the fixture. Axiom's single-pass floor planner uses absolute rows across
regions, so the source cells were moved beyond all hash rows. `065746Z` also
recorded concurrent main Cargo.lock drift and failed its qualification guard.
`070517Z` passed all seven repaired controls. `070642Z` passed 7/8: direct
out-of-range MockProver assignment panicked instead of returning a row-capacity
error; explicit preflight repaired the harness. `070825Z` passed 8/8 before
the final style-only correction. No failed evidence was overwritten.

The [semantic design](../../../specs/ram_lfe_semantic_commitments.md) remains
proposed. This experiment does not add a relation identifier, production caller,
opening authority, BFV security claim or proof-mode admission.
