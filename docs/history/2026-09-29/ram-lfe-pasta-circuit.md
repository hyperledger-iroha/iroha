# Pinned Pasta circuit experiment

Date: 2026-09-29. Ten native controls pass, with zero ignored tests, for the
paired-partial-round layout in an isolated harness compiling the exact Core
circuit/adapter sources and shared native leaf. The maximum single hash now
fits k=16 and produces a 2,688-byte IPA proof. Its measured verification time is
238.840 ms, above the unchanged 20 ms soft budget. This remains an unused
test-only primitive, not a qualified RAM execution relation. Production
57-round helpers, keys, roots, limits and proof admission remain unchanged.
The known-insecure diagnostic BFV profile requires separate replacement.

## Constraints and controls

`crates/iroha_core/src/zk/ram_lfe_poseidon.rs` implements exact upstream
8 + 56 original Poseidon and `ConstantLength<L>` over Fp and Fq. Constants come
only from `iroha_zkp_poseidon::pasta`. Full rounds retain one row each. A partial
pair reuses the first input advice cell as `y = (s[0] + c[0])^5`; the first MDS
mix is linear in `y`, `s[1] + c[1]` and `s[2] + c[2]`. The second partial S-box
and MDS constrain the next state directly. The gates remain degree six. No
host hash callback supplies a constraint.

The ten controls cover all 22 upstream two-input vectors, odd/even lengths,
432 propagated arbitrary-field state-boundary mutations and 112 propagated
first-S-box mutations across both fields and two absorption blocks. Coordinated
initial-state, absorption, input, copy and padding mutations remain covered.
Mutation tests omit public output binding so it cannot conceal a missing local
constraint. Owned cells clear on success, error and unwind, including after
the first partial S-box has been assigned. Sample and maximum genuine Fp IPA
proofs reject changed public instances, tampering and trailing bytes. Both
fields pass maximum-size MockProver parity and refuse k=15 before domain-sized
allocation. No Fq IPA proof is claimed.

## Measured geometry and resources

The circuit has five advice columns, eight advice queries, six round-constant
fixed columns/queries before selector compression, no lookups, six permutation
columns, five blinding factors and eight minimum rows. Its five selectors are
allocated separately during configuration. The fixed layout uses eight full
round rows plus 28 paired partial rows per permutation. A hash of L fields uses
`37 * ceil(L / 2) + 1` rows, followed by L disjoint source-copy rows.

At the maximum 2,054 fields this is 38,000 hash rows, 2,054 source rows and eight
required spare rows: 40,062 in total, within 65,536. The earlier layout required
66,756 hash rows and k=17. Fitting this one hash does not establish capacity for
the complete semantic/interpreter relation.

| Fp control | Sample | Maximum |
| --- | ---: | ---: |
| Fields / k | 3 / 8 | 2,054 / 16 |
| Hash rows | 75 | 38,000 |
| Proof bytes | 2,176 | 2,688 |
| Processed verifying-key bytes | 554 | 554 |
| Parameters and key generation | 62.839 ms | 23,281.803 ms |
| Proving | 57.685 ms | 8,501.969 ms |
| Verification | 3.018 ms | 238.840 ms |
| Test-process peak RSS | 7,716,864 bytes | 328,794,112 bytes |

These are individual optimized native measurements, not production latency
qualification. The prior paired run also measured maximum verification at
233.140 ms. The existing single-proof verifier expands `2^k` challenge scalars
and performs the full IPA MSM check; the measured latency exceeds 20 ms even
for this primitive. Proofs remain below the existing 192 KiB cap. No limit was
raised, and no complete envelope or full RAM proof was constructed.

Owned working scratch remains eight field elements (256 bytes). Input bytes
use a clearing shared owner, at most 65,728 bytes. Tests observe actual cells
after zeroization. Caller copies, arithmetic/compiler temporaries and Halo2's
witness/prover buffers are outside this guarantee.

## Immutable evidence

Final paired evidence is under ignored
`dist/zk-remediation/2026-09-29/ram-lfe-circuit/poseidon-paired-20260929T112238Z`.
The fresh isolated target compiled all seven local compiler artifacts anew
(`fresh=false`); their manifest and output hashes are retained. All 349 source
inputs and 352 actual captured compilation paths match before/after. The copied
test binary is unchanged. All ten selected names run individually exactly once,
with zero ignored tests. Total fresh build, native controls and lint took
247.800 s. This uses a standalone manifest with optimized test/dev profiles,
two build jobs and four Rayon threads; normal Core integration of this new
layout is pending.

- Binary: `cb0edbd243d27fe1fa3e011d586ec03c35e9faa1df8968b2a020a2d014da47cb`.
- Circuit: `d14d56208fce7cf037fd4646f6022003844b94d81c24279219eec25376e34aad`.
- Tests: `6ab3a86563fba252ec6f28dc7e6dd646c716fbc43e0dc461f45885471176bff3`.
- Shared native leaf: `643defb895437930c961ea94203f5f190f0fc90a5bfb29dfee7f4808353da8fb`.
- Halo2 adapter: `14cdcb21686c54444c53f35903f46a6bb577238d24794995371b65aafe6c44c6`.
- Generated lock: `d945054a08740c3d0ae779020b70a1cd41e4338eb6639feffa68b9f339056e8c`.

Rustc/Cargo 1.93.1 version reports and binary hashes are recorded. No inherited
flags, compiler wrappers, build-target, profile or stack overrides were present.
Scoped rustfmt and whitespace checks pass. `clippy -- -D clippy::all` passes;
five adapter dead-code warnings remain because this harness uses only part of
that module. This is not a `-D warnings` qualification.

## Retained predecessors

The first paired run, `poseidon-paired-20260929T111731Z`, passed 9/10 with
Clippy and all source/binary guards passing. Only the geometry fixture failed:
it incorrectly counted five selector columns before selector compression.
The actual API reports six explicit fixed columns. The repair changes only
those two count assertions; the failed receipt and maximum-proof measurements
remain intact.

The one-round-per-row predecessor `poseidon-20260929T072413Z` retains 8/8,
Clippy and immutable source/binary results. Its sample proof was 2,080 bytes,
VK 458 bytes, key generation 62.065 ms, proving 65.876 ms, verification 2.798 ms,
and overall native peak RSS 212,959,232 bytes. Both fields needed k=17 for the
maximum record. Its exact original dated record and source are preserved under
`ram-lfe-circuit/paired-partial/before`.

Earlier retained attempts: `065248Z` failed generic trait-bound compilation;
`065746Z` and `070338Z` passed 6/7 and exposed source/absorption cell aliasing;
`065746Z` also recorded concurrent lockfile drift. `070517Z` passed all seven
after separating source rows. `070642Z` passed 7/8 and exposed direct MockProver
out-of-range panic; explicit preflight fixed that. `070825Z` passed 8/8 before
a style correction. `071041Z` passed 8/8 while Clippy rejected the adapter's
former explicit transcript drop. `072413Z` passed after lexical scoping fixed
that borrow release. No failed evidence was overwritten.

The old normal Core capture `boundary-native-20260929T074949Z` observed all
eight predecessor controls passing but reused local objects without retained
provenance. A later coherent candidate reran those eight controls successfully
under `bfv-api-components-20260929T093117Z/native-controls-stage0-retry1` with
source/binary guards and artifact lineage; its broader suite retained two
unrelated fixture failures. Neither receipt qualifies the new paired layout.

The [semantic design](../../../specs/ram_lfe_semantic_commitments.md) remains
proposed. This experiment adds no production relation identifier, caller,
opening authority, BFV security claim or proof-mode admission.
