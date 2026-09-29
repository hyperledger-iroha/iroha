# FASTPQ production readiness

Updated: 2026-09-29. **Production qualification is unavailable.** Canonical masked
quantity proving and bounded verification are implemented. Local proof tests,
resource plans and hardware measurements establish only their captured scope;
they do not replace independent cryptographic or deployment qualification.

The [DEEP protocol contract](fastpq_deep_protocol_contract.md) owns the current
relation, geometry, transcript and wire rules. The
[implementation plan](fastpq_plan.md) owns remaining outcomes. This record
summarizes current evidence; exact commands, source captures, binaries, original
failures and earlier measurements belong in the
[September 28 validation record](../docs/history/2026-09-28/fastpq-masked-native-validation.md).

## Current masked integration

Core ordinary transfers and AXT envelopes use the canonical masked quantity
artifact verifier through `fastpq_prover::offline_compact`. The fixed child has
65,536 trace rows, 301 committed private columns, 41 reconstructed public
columns, 923 AIR slots, 8,388,608 evaluation rows and 64 distinct queries. Its
five FRI folds use fixed arities `[16,16,8,8,4]`; the degree progression
`[131072,8192,512,64,8,2]` binds the final degree-`<2` check. The exact maximal
canonical frame is 502,895 bytes, including the composition-mask opening.

Fresh cryptographic entropy supplies subgroup-vanishing trace masks, paired
quotient masks and an independently committed composition mask. The integrated
profile uses 136 base-field mask coefficients per private trace column and 65
extension-field quotient-mask coefficients. The complete relation is evaluated
at the OOD point; exact numerator division rejects nonzero remainders.
Independent whole-transcript hiding, soundness and Fiat–Shamir/qROM review remain
required. Public statement values are not hidden.

Each verifier derives its complete expected statement from independently
authenticated context. Ordinary roots cover the touched-balance tree, not the
complete persisted ledger. AXT artifact consistency additionally binds its
outer context, but does not authenticate source finality or authorize remote
spending. Core rejects unanchored remote spending and standalone non-null
`AXT_VERIFY_DS_PROOF` promotion. Finalized relay and fee-vault paths retain
separate authorization and source-state checks. Opaque metadata carriers cannot
acquire transfer authority; catalogued mint, burn and other operations need their
own complete relations.

## Developer and resource contract

The normal quantity API owns circuit, transcript, masking and geometry choices.
Callers supply a statement, its independently derived `ExpectedStatement` and
explicit proving/verification limits. The producer verifies its output before
returning. The public [developer guide](https://docs.iroha.tech/blockchain/fastpq)
and crate Rustdoc demonstrate this workflow. `prove_axt_bound_batch` exposes the
same canonical producer directly for bound batches. Transparent `Proof`,
`Prover` and replay verification are restricted to tests and `dev-tools`, with
no normal-library compatibility aliases. Binding rejects a transfer transcript
from another source before changing metadata. Normal API, precise preflight,
replay regression and compile-fail controls pass. The optimized two-child
ordinary and AXT facades and their independent retained-artifact replays pass.

`quantity_artifact_resources(segments, maximum_segment_statement_bytes)` provides
witness-free planning. A zero statement length gives a floor, not an exact
admission result. Actual proving separately admits complete contexts, private
SMT source dimensions and checked allocation/work limits before private work.
Whole artifacts and cumulative decoding have additional bounds.

Defaults remain 524,288 bytes per child, 2 GiB charged construction payload per
segment and 2^42 structural work units. The current fixed-SMT plan charges
1,967,970,414 bytes, 3,475,021,175,280 work units and 34,689,999 hash calls.
It includes the complete 64 MiB retained Metal pool during the CPU quotient
phase as well as digest dispatches. These are structural charges, not process
RSS reservations or latency promises.
The same limits include retained internal Merkle nodes, coverage, pending
owners and opening scratch. Same-attempt caches bind immutable context and the
original roots; selected leaf stripes are regenerated and every opening must
reconstruct its committed root before disclosure.

Only one producer runs per process; bundle segments are sequential. Applications
run synchronous proving on a worker and handle `ProvingError::Busy` using a
bounded queue or retry policy. CPU is the default. Explicit required-device
execution checks availability before private work and never silently falls back.
Metal accelerates bulk leaf and lower-parent hashing; this is not whole-proof
GPU execution. Unknown device completion quarantines retained buffers and blocks
future proof admission in that process; recovery requires a fresh process.

## Captured proof evidence

| Captured artifact | Size | Whole-test wall time / maximum RSS | Scope |
| --- | --- | --- | --- |
| Pre-source-release cached fixed-SMT child | 482,978 bytes | 909.44 seconds / 1,919,795,200 bytes | Required Metal hashing with CPU arithmetic; 907.486 seconds construction plus self-check; complete seeded bytes match the pre-cache child. |
| Separate replay of that retained child | Same public bytes | 2.15 seconds / 19,644,416 bytes | No witness construction or prover; independent expected statement, cap/context/tamper rejection and five changed-statement controls. |
| Pre-source-release cached fixed-SMT child, CPU | Same 482,978 bytes | 2,532.67 seconds / 1,895,317,504 bytes | Same immutable executable, seed, statement and default limits as Metal; complete bytes match; separate artifact replay passes in 2.07 seconds. |
| Pre-cache ordinary one-child facade | 485,600 bytes; child 483,777 | 5,250.63 seconds / 1,110,228,992 bytes | Complete public API generation, self-verification and retained independent replay. |
| Pre-cache AXT one-child facade | 484,750 bytes; child 481,729 | 2,049.67 seconds / 1,109,753,856 bytes | Complete public API generation, self-verification and retained independent replay. |
| Optimized cached ordinary two-child facade | 968,475 bytes; children total 965,116 | 1,735.04 seconds / 2,174,222,336 bytes | Actual FASTPQ opt-level 3, Required Metal hashing, default structural limits; 1,726.794 seconds construction plus self-check. |
| Separate replay of that ordinary artifact | Same public bytes | 7.62 seconds / 29,687,808 bytes | Independent fixed caller statement and the complete retained-artifact rejection controls, without reproving. |
| Optimized cached AXT two-child facade | 971,571 bytes; children total 966,608 | 1,741.24 seconds / 2,087,878,656 bytes | Same immutable opt-level 3 executable and default structural limits; 1,727.694 seconds construction plus self-check. |
| Separate replay of that AXT artifact | Same public bytes | 14.00 seconds / 37,371,904 bytes | Independent fixed caller statement and complete retained-artifact rejection controls, without reproving. |
| Source-release repair, fixed-SMT child | Same 482,978 bytes | 2,421.09 seconds / 1,765,064,704 bytes | Opt-level 3 unit diagnostic, Required Metal; 2,416.611 seconds construction plus self-check; exact retained baseline byte equality. |
| Separate replay of the repaired child | Same public bytes | 2.09 seconds / 19,234,816 bytes | Current immutable verifier, independently derived statement and all prior cap/context/statement/tamper controls. |

The byte-identical fixed-SMT child SHA-256 is
`6b0f68181920c7c506c949f5bea7bee9ba9951b0c7142837a536ef1f87020b19`.
The earlier CPU/Metal pair's immutable test executable SHA-256 is
`eb19553839df39e50f53880046ec7afd0bf7b99dbc9b7bac6661df52addbda9a`.
The 222-file FASTPQ/ISI/build capture SHA-256 is
`c0500bcf5a06272a4f7296cdea948b30bec6fcf81162e0cda5e59b22fd5459d2`;
this is a scoped capture, not a complete dependency-closure attestation. Public
proofs, receipts and binaries are retained in
`dist/zk-remediation/2026-09-28/fastpq/`. No private witness, mask or coefficient
is retained in public proof receipts.

The saved one-child ordinary and AXT artifacts verify on later captured verifier
binaries without reproving. Their generation timings describe the pre-cache
producer. Those runs and the fixed-SMT child runs use Cargo `test` builds:
FASTPQ Rust callers/FFT are unoptimized (opt-level 0), while
`fastpq_isi` has its package opt-level 2 override. The build-profile receipt and
Cargo fingerprint are retained with the evidence. The optimized two-child
ordinary artifact has SHA-256
`66130f25f755e702e87ab6161fc956afe53059fe5e3d186cad38890e606055cf`;
its immutable binary has SHA-256
`c994d5722bcaa18fedf666371359f4e4e09b4fb927e814e93e64e4b501004986`.
Its 223-file FASTPQ/ISI/build capture, actual opt-level 3 rustc invocation,
public artifacts and independent replays are retained under
`dist/zk-remediation/2026-09-28/fastpq-two-child-release-run1/`.
The AXT artifact SHA-256 is
`ea2aa128229045c6e8d3c990bbbb7e590d313f0d2622ba7f9321ffdccd7f7246`;
the final source seal has zero drift. Ordinary RSS exceeds 2 GiB by 26,738,688
bytes, although AXT RSS remains below it. Passing structural admission does not
establish a 2 GiB process-RSS bound. Early physical-source release and complete
Metal pool charging are implemented. The physical source now occupies one
clearing matrix and is consumed before row commitments; native controls compare
every masked coefficient and the entropy position and observe actual-cell
cleanup on success, failure and unwind. The final optimized selection passes
39 ownership/resource/producer controls, four separate exact-root resource
controls and 15 public API controls. The repaired seeded child now matches the
retained baseline bytes and passes independent artifact verification, with
1,765,064,704 bytes maximum RSS. Its opt-level 3 unit executable has SHA-256
`193a54c2e050cd1a067c8c5eafb3aa7c4745fc7a46f41de04f41a5343b8f8403`;
225 scoped source inputs have zero drift. Receipts are retained under
`dist/zk-remediation/2026-09-29/fastpq-source-owner-proof-run1/`.
Both repaired-source two-child remeasurements completed: ordinary produces
971,675 bytes at 1,881,849,856 bytes maximum RSS; AXT produces 973,573 bytes at
1,875,820,544 bytes maximum RSS. Each passes separate retained-artifact replay.
The unchanged structural bounds still apply. These measured process outcomes
resolve the observed overrun for the recorded repeated-key fixtures, not a
universal RSS guarantee. Their maximum default two segments/four update
occurrences use a sender near the signed 512-bit maximum and receiver scale 28;
repeated ALICE/BOB keys do not cover every distinct-key/touched-tree shape.
The proof-run source check records only `Cargo.lock` drift after capture; it
cannot attest the subsequently changed checkout or complete dependency closure.
The [September 29 record](../docs/history/2026-09-29/fastpq-two-child-memory-repair.md)
retains artifact identities and exact scope. Four-distinct-key and maximum AXT
context tests are implemented but await a fresh build and complete proofs.
Maximum application shapes and broader deployment resource behavior remain open.
All quoted timings are from a contended host and describe the stated fixtures.
They do not establish a controlled speed ratio or release-fleet throughput.

The cache suite initially passed 203 controls and failed one older opening
fixture; the rebuilt integration selection passes both controls after repairing
that fixture. The subsequent retention/cache/resource selection passes 14
controls; its external API selection passes 12, and its actual required-Metal
selection passes three. These overlapping selections are not one aggregate
count. Earlier complete 8M-point FFT/inverse and exact-root transform oracles
also pass with their own captures.

Exact-size CPU/Metal kernel parity, complete pre/post-cache Metal proof-byte
parity and complete same-fixture CPU/Metal proof-byte parity pass. The CPU run
uses the same immutable executable, seed, statement, caps and golden assertions;
its content-addressed writer compares the actual bytes with the retained Metal
artifact before accepting the existing file. Separate artifact verification and
mutation controls also pass. The September 26 CPU proof remains separately
scoped to a different fixture/candidate. Maximum-shape facade measurements, corrected whole-process memory behavior,
CUDA hardware and release-fleet qualification remain open.

## Completion goals

| Owner | Required outcome | Remaining evidence |
| --- | --- | --- |
| API | One canonical normal proving/verification workflow | Direct AXT producer and diagnostic-only replay cut applied; normal API 15, AXT 53, dev-tools integration 28, transcript 1 and Rustdoc 6 controls pass in their stated selections. Optimized ordinary and AXT two-child generation/replay pass; frozen network consumer checks remain pending. |
| Relation and resource owners | Complete supported application shapes within unchanged limits | Two-child ordinary/AXT proofs and independent replays pass; maximum application shapes, corrected whole-process memory under the unchanged cap, cumulative decode/preparation bounds and fleet time/RSS remain open. |
| Hardware owners | Deterministic complete proofs and bounded failures | Preserve complete same-fixture parity while measuring optimized public facades; qualify actual device cleanup/quarantine and supported hardware. No missing-device skip counts as a pass. |
| Cryptographic reviewers | Independently qualified masked protocol | Artifact-bound soundness, hiding, Fiat–Shamir/qROM, concrete six-lane digest/multi-target and arithmetic/side-channel analysis. |
| Core/Nexus | Authorized source-state and business effects | Exact finalized roots, intent, proof-bound hidden amounts, durable atomic spend nonces, budget/custody checks, restart/recovery and four-validator adversarial admission on one candidate. |
| Release owners | Reproducible source, artifacts and consumers | Fixed source/lockfile/toolchain, native and SDK consumers, signed provenance, applicable workspace/Clippy tests and network qualification; failures and unexecuted checks remain explicit. |

The [hiding construction](fastpq_deep_hiding_construction.md) and
[digest qualification boundary](fastpq_compact_digest_security.md) state the
remaining cryptographic obligations. The six-lane canonical output space is
`p^6`, not exactly `2^384`. All 32-byte external commitments need separate
analysis; `iroha_crypto::Hash` fixes one marker bit, leaving 255 variable output
bits. Width, local algebra or an implementation cross-check is not an
independent security argument.

The [ZK first-release goals](zk_first_release_goals.md) coordinate wallet, Vega,
X509 and FASTPQ completion. Full replay or an unmasked proof cannot satisfy the
private succinct-proof goal, and increasing limits cannot substitute for the
required construction. Historical designs, commands and evidence are preserved
verbatim in the [pre-reconciliation record](../docs/history/2026-09-28/fastpq-readiness-before-deep-reconciliation.md);
those earlier geometry and replay claims do not describe the selected protocol.
