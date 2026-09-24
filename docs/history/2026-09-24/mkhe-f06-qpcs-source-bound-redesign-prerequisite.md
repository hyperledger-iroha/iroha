# F06 native40 qPCS redesign prerequisite, 2026-09-24

This is a source-only feasibility and interface audit on `optimizations`. It
does not propose a second accepted proof, change any limit, or qualify native40.
No Cargo command was run for this audit. The current constructor and composite
admission must remain fail-closed.

## What the present geometry rules out

The initial oracle contains `2^19 = 524,288` canonical 6,000-byte leaves,
each carrying 400 Fq2 coordinates (ten rows for each of 40 distinct RNS
limbs), and `524,287` binary internal nodes. The shared six-lane frame charges
4,819,350 tracked Goldilocks adds/multiplies per payload hash, 487,008 per
index-bound leaf hash, and 476,862 per node hash. These are the exact frame
costs used by `RnsNativeTreeWorkV1::for_oracle` and charged by
`RnsNativeQpcsTreeV1::build` *before* source access. Therefore:

| Initial-tree component | Exact tracked operations |
|---|---:|
| All 6,000-byte payloads | `524,288 × 4,819,350 = 2,526,727,372,800` |
| Index-bound leaves only | `524,288 × 487,008 = 255,332,450,304` |
| Binary nodes only | `524,287 × 476,862 = 250,012,547,394` |
| Complete current tree | `3,032,072,370,498` |

The whole-proof ceiling is 128,000,000,000 tracked operations. **Each of the
three present components individually exceeds it.** Thus removing one layer,
compressing bytes, caching public payloads, or changing only tree arity cannot
make the unchanged other layers admissible. If *all other proof work were
free*, a per-initial-leaf operation would have at most
`floor(128,000,000,000 / 524,288) = 244,140` units; real source, openings,
FRI, and cross-field work make its allowed share smaller. This ceiling is a
screen for a candidate construction, not a lower bound for every possible
commitment scheme. The existing checked index-plus-node floor and test already
cover the current constructor; a duplicate Rust budget helper would not be a
useful prerequisite.

The present wire's initial and quotient multiproofs each bound 320 complete
opened leaves plus at most 3,392 authentication digests:
`320 × 6,000 + 3,392 × 48 = 2,082,816` bytes per tree. The current qPCS
upper bound is 29,312,055 bytes before its mandatory source continuation,
leaving only 1,428,297 bytes under the 30,740,352-byte qPCS section limit.
The complete envelope remains capped at 41,943,040 bytes, resident memory at
512 MiB, authenticated spool at 16 GiB, and authenticated I/O at 64 GiB.
These byte inequalities are not measured whole-proof lifetimes.

## Smallest reviewable redesign slice

Specify **one source-bound commitment and evaluation-opening relation** before
replacing any root or parser. For limb `l` and row `r`, the authenticated
source must determine the committed low-degree `f[l,r]` over that limb's
`Fq_l^2`; no independently chosen aggregate codeword may stand in for it.
For each of the five relation points `z[l,floor(r/2)]` per limb, the proposed
opening must prove, at verifier-derived queries `x`, the current equation

`f[l,r](x) - y[l,r] = (x - z[l,floor(r/2)]) * g[l,r](x)` in `Fq_l^2`,

with `y`, `g`, the source commitment, and the exact query roots bound in the
same transcript chronology. The existing scalar relation also checks
`product = (z^131072 + 1) * quotient` in `Fq_l`. The current FRI-0 batched row
is `a * x^(131072 * (r mod 2)) * f(x) +
b * x^(1 + 131072 * (r mod 2)) * g(x)`; a replacement must preserve or
soundly replace that link to all 400 rows. Directly lifting residues into one
Fp4 word per position is insufficient: the 40 base characteristics differ,
and a detached aggregate commitment can satisfy its own Merkle/FRI checks
without matching the native rows.

The first implementation interface to review is a source-derived
commitment/opening relation **inside the existing move-only authority**, not
a parallel owner. The present `authenticate_qpcs_candidate_v2` compares
context digests and borrowed slice identity against a retained qPCS stage; it
does not prove that a replacement commitment's 400 codewords equal the
polynomials derived from the authenticated 43-record opening inventory.
That missing relation must retain the existing
`RnsNativeQpcsRelationScheduleV1` and
`RnsNativeQpcsFriCompleteStageV1` lineage and be consumed at
`authenticate_qpcs_candidate_v2` inside
`verify_zk_ams_mkhe_rns_native_composite_from_source_chain_v2`. The public
standalone verifier must still return `StageUnavailable`. Before code, the
design needs exact commitment bytes, evaluation equations for **each** limb,
challenge order, opening/error bounds, a 128-bit soundness and privacy
argument, a permitted six-lane hash schedule, and an additive work/lifetime
ledger in the existing units and caps. Tests must adversarially swap source
records, limbs, rows, evaluations, roots, and query schedules while retaining
otherwise valid openings, then measure full40/eight-party resource bounds.

No safe non-cryptographic code change was found in this slice: the present
cost floor and admission already reject before source access, while the
remaining change is the cryptographic statement itself. Do not delete the
current quotient/FRI-0 commitments, reinterpret their roots as tags, or admit
a size-only one-field aggregate before the replacement construction and its
source/composite proof are specified and reviewed.

Source anchors: [`rns_native_qpcs_tree.rs`](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_qpcs_tree.rs),
[`rns_native_qpcs_leaf.rs`](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_qpcs_leaf.rs),
[`rns_native_qpcs_prefix.rs`](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_qpcs_prefix.rs),
[`rns_native_composite_verifier.rs`](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_composite_verifier.rs),
and the [fixed wire contract](../../../specs/crypto/zk_ams_rns_native_wire_v1.md).
