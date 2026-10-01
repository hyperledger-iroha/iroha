# ZK first-release completion goals

Set: 2026-09-26. Execution resumed: 2026-09-30. Reviewed: 2026-10-01. Overall status: **Active**.

This record owns the remediation requested after the current-source ZK critique.
It supplements [first-release completion](first_release_completion_goals.md) and
[privacy closure](privacy_first_release_closure.md). Implementation remains in the
existing `/Users/takemiyamakoto/devstuff/iroha` checkout on `optimizations`.
All further implementation and validation use this checkout; earlier separate
candidates are historical evidence only. Unrelated changes remain.

The outcome is useful, safe proof algorithms with simple developer APIs. Renaming,
disabling, status objects, source hashes or relaxed caps do not complete an algorithm.
Dedicated review uses independent derivations and adversarial controls bound to
the actual artifacts. Implementation tests alone do not establish cryptographic
qualification; physical-device and signing claims require their actual runs.

The [September 30 continuation](../docs/history/2026-09-30/zk-completion-continuation.md)
records the native/SDK provenance, current validation and pending work.
The six unfinished outcomes remain ZK03 through ZK08; no running build, source
review or partial test selection closes one of them.

## Goals and acceptance criteria

| ID | Status | Owner / outcome | Completion criteria |
| --- | --- | --- | --- |
| ZK01 | Complete (implementation) | Vega / secret-safe proving | Every private commitment uses public work dimensions and constant-time secret arithmetic; no raw uncleared witness copies cross the MSM boundary. Differential arithmetic, zero/partial/full row, failure/cleanup and worker-bound tests pass. Report resource limits only when actually enforced. Review other reachable secret hashing scratch, including ZK-ACE. Target timing and independent qualification remain ZK06. |
| ZK02 | Complete (implementation) | Confidential circuits / optional inputs | One owned note can be fully redeemed at maximum tree capacity. Absent inputs require no empty-leaf membership or caller-created dummy witness. Ownership, nonzero/duplicate-nullifier, range and conservation constraints remain enforced. Regenerate all changed circuit keys, digests and dependent fixtures; reject superseded keys. Positive and adversarial circuit/native/SDK tests pass. |
| ZK03 | In progress | Core / honest proof semantics | Generic verification cannot confer a stronger guarantee than its compiled relation. Retire IVM binding-only circuits, registrations, keygen, service routes and SDK/CLI consumers. Production IvmProved admission requires the complete native execution relation and authoritative finalized State anchor; replay or caller-supplied commitments cannot substitute. Implement the complete RAM-LFE program relation before enabling proof receipts. |
| ZK04 | In progress | FASTPQ / bounded private verification | Implement a sound source-state-bound relation with reviewed trace/composition masking and bounded verifier work; fit unchanged proof and total resource limits. Produce and verify real maximum-shape proofs, negative source/witness/statement cases, and CPU/accelerator parity. Full replay and unmasked offline compact proofs do not satisfy this goal. |
| ZK05 | In progress | ZK-X509 / complete bounded credential proof | Redesign or compose the full certificate, CRL, disclosure and ownership relation below 9,437,184 bytes without reducing supported coverage. Account for all segments, recursion, openings and prover resources together. Regenerate fixed profiles and produce actual positive/negative proofs before activation. Arithmetic size projections alone are insufficient. |
| ZK06 | In progress | Cryptographic qualification | Obtain independent artifact-bound soundness, zero-knowledge, Fiat–Shamir/qROM, digest/multi-target, arithmetic and side-channel evidence for the selected release protocols. Keep explicit protocol-specific blockers; do not infer qualification from implementation markers. |
| ZK07 | In progress | SDK / simple developer workflow | One typed prepare/prove/verify workflow per actual capability, actionable errors, early availability/resource checks, secure randomness and private witness ownership by default. Callers do not construct dummy inputs, choose transcript internals, or infer guarantees from backend names. Maintain executable Rust examples and equivalent supported SDK entrypoints, with public guidance in `iroha-docs`. |
| ZK08 | In progress | Validation / reproducible candidate | Reconcile current failing Halo2/note-STARK source contracts by preserving their substantive assertions. Run changed-function tests, real proof/adversarial suites, serialization guards, formatting and applicable SDK tests; then the workspace and four-validator qualification on a fixed candidate. Record exact passes, failures and unexecuted checks separately. |

## Execution order

1. Repair secret arithmetic and optional-input constraints with regression tests.
2. Make guarantees explicit at verifier/developer boundaries; migrate callers and remove stale claims.
3. Use one allocation-free resource plan at developer preflight and actual prover
   admission. Resolve the mathematical construction before widening execution.
4. Complete FASTPQ and X509 constructions against fixed statement and resource
   contracts, then regenerate profiles, keys and fixtures.
5. Qualify end-to-end SDK/native/network paths and independent cryptographic
   evidence. Close each goal only with its acceptance evidence.

## Current implementation and evidence

The completed epoch10 native, SDK and network evidence below is historical. A
subsequent merge on `optimizations` changed compiled inputs while three remaining
control runs were active. Their native events and public artifacts are retained,
but their source guards fail; they qualify no successor. Merge conflict resolution
has finished and the reconciled repairs are applied. The merge remains uncommitted
because the local signing executable is unavailable. The exact uncommitted
candidate requires new normal builds, genuine fixtures, native proofs, SDK
consumers and integrated checks. No partial selection closes a goal.

### ZK01 — secret arithmetic

Vega private commitments use fixed-window secret MSM, public dimensions and
clearing scalar owners. ZK-ACE borrowed private preimages and reachable hashing
scratch clear. Accelerator allocations clear only after observed completion;
unknown completion quarantines their owners and blocks further private admission.
Compiler-created copies, device timing and independent review remain ZK06.

### ZK02 — optional confidential inputs

Absent inputs require no empty-leaf path. One owned note can be fully redeemed at
the 65,536-leaf maximum, or redeemed with private change. Ownership, range,
conservation and nullifier constraints remain enforced. Canonical replacement
keys reject superseded keys. Same-candidate network, distribution and device
qualification remains open.

### ZK03 — complete proof semantics

`zk::verify_for_relation` admits only an explicit role supported by its compiled
relation. IVM replay-binding circuits, registrations, keygen, service routes and
associated SDK/CLI consumers are retired. Production `IvmProved` admission stays
closed until the complete native transition relation and authoritative finalized
State binding exist; G3 in the [IVM goals](kotodama_ivm_completion.md) owns this work.

RAM-LFE registration, activation, restoration, evaluation and receipts reject the
insecure signed/proof BFV modes before private work. The exact-lift profile loses
public-key noise modulo 257. The diagnostic interpreter's clearing tape, bounded
initializer, reduction and commitments are prerequisites, not an execution proof.
Secure encryption replacement, exact refresh, malicious-key/input validation,
circuit privacy, the complete program relation and integrated qualification remain
open under the [replacement contract](ram_lfe_encryption_replacement.md).

The candidate [packing](ram_lfe_plaintext_packing.md) and
[semantic](ram_lfe_semantic_commitments.md) interfaces preserve private length,
ordered scalar output including 256, clearing owners and explicit public
policy/key/profile/context binding. They authenticate no submitted key or proof.
Input and proof-envelope caps remain independently 1 MiB, with the configured
proof cap at 192 KiB. Input and output ciphertexts do not share the same input
frame; complete replacement proof encoding and total working memory remain open. Retained public arithmetic/parameter research
does not establish a complete refresh or qualified encryption construction.

The historical IVM quick selection passes 114 native controls. Three of the 19
separate scalar/AIR proof controls complete natively; the third crosses source
changes, and the remaining 16 are unexecuted. No 133-control union is admitted.
The owner-summary fixture has one native failure from a missing unit-result slot;
its prepared correction requires current native validation. Reviewed LOAD/STORE
banks preserve native logging, trap, alias and write-order behavior, but remain
unregistered prerequisites. Memory authority, initialization/history, execution
sequencing, integrated resource accounting and finalized State binding remain open.

### ZK04 — FASTPQ

The public Quantity facade uses the masked bounded DEEP producer and independent
verifier. Expected statements derive from canonical authenticated context; proof
consistency alone does not authorize remote spending. Development replay APIs and
required-Metal readiness preserve their existing boundaries.

The historical 77-query SHA3/SHAKE library selection passes 1,315 native controls,
including all six known-answer controls; 16 offline ordinary controls also pass.
The seeded CPU and required-Metal proofs pass positive/negative verification and
authentic golden-byte checks, producing identical 485,219-byte proofs. Its evidence wrapper initially rejected the
native diagnostic path spelling; a separate source-bound reevaluation preserves
that wrapper failure and authenticates the real proof and receipt. One maximum ordinary producer completes natively and retains a 973,318-byte
proof, but the source changes during that run. Its wrapper also rejects a stale
child-frame expectation; a strict parser repair does not repair source provenance.
Independent replay and the remaining maximum pairs are unexecuted. No earlier
maximum result qualifies the new merged candidate.

[FASTPQ readiness](fastpq_production_readiness.md) owns unchanged proof, payload
and work limits. Dedicated derivations provide conditional classical-ROM and
ideal-QROM bounds under explicit query/attempt assumptions. They do not establish
concrete Keccak security, full zero-knowledge, device/side-channel behavior or
finalized authority. Finalized-source network behavior, broader workload shapes
and current hardware coverage remain open.

### ZK05 — ZK-X509

MAIN retains all 49 registrations and certificate, CRL, disclosure and ownership
coverage. The maximum encoding is 9,420,938 bytes against the unchanged
9,437,184-byte ceiling. Geometry remains 285 base / 280 auxiliary / 102 fixed
columns, 1,681 constraints and degree four; native temporal and padding repairs
preserve the supported relation and rejection of superseded profiles.

The historical normal optimized binary passes 184 focused native executions over
183 distinct tests, with no failures or skips. The complete maximum credential
produces a verified 9,420,938-byte proof; self-check, public verification,
wrong-genesis and tampered-proof controls pass. A separate verifier process accepts
the exact retained proof without regeneration. Peak producer RSS is 9,593,044,992
bytes, below 12 GiB. Proving takes 2,925.309071 seconds against 300 seconds, so the
maximum test fails and activation remains unavailable. The public proof SHA-256 is
`f24b64f44345c015d2a76b7c038b3c8effab42f60a415fce284a2f6df573d9ae`.

The original maximum wrapper stopped on repeated append-only receipt metadata;
its failure remains retained. Separate recovery validates that exact receipt and
runs the standalone verifier under the unchanged historical source and binary admission. Evidence
is under `dist/zk-remediation/2026-09-30/x509-epoch10-maximum-proof1` and
`x509-epoch10-retained-replay-recovery1`. Concurrent work and two bounded stack
samples preclude a causal performance comparison with earlier runs.

Measured phases include 1,128.98 seconds in composition and 662.83 seconds in
query openings. Reviewed successor work factors Metal twiddles and projects only
requested arithmetic auxiliary families, skipping inactive fixed public rows.
The dense oracle, exact terminal ownership, clearing/error behavior, coverage and
limits remain unchanged; native parity and another full maximum run are required.
The historical exact-root CPU/Metal parity diagnostic passes at native19/common22
geometry; its timings do not establish the successor's speedup.

Public terminal-product claims expose values not covered by trace masking. A
protocol repair and artifact-bound zero-knowledge review are required alongside
performance work. A faster proof alone cannot clear activation.

### ZK06 — cryptographic qualification

Dedicated independent source reads, derivations and adversarial controls are
recorded against exact protocol artifacts. Their scope is explicit: component
arithmetic, conditional soundness/hiding bounds and authentic native verification
do not establish complete soundness, zero-knowledge, transcript security,
side-channel resistance or release qualification. RAM-LFE construction and full
IVM semantics remain absent; X509 has the terminal-claim privacy and time blockers.
Physical hardware and authenticated final release artifacts require actual runs.

### ZK07 — developer workflow

Typed wallet workflows own clearing private inputs, canonical keys and circuit
selection; callers do not supply dummy inputs or transcript internals. JavaScript,
Python, Swift, Kotlin/Java and C# use the shared native owner with bounded inputs
and asynchronous custody. Managed strings do not promise erasure.

Historical host ABI-25 controls pass 62 tests with 397 expected exports, plus ten
top-up executions. Kotlin records 1,562 passes and six full-tree controls; Android
host/JNI consumers pass 291 tests. C# records 6,015 passes. Installed Python passes
4,370 tests plus 281 subtests and five genuine maximum-tree wallet controls.
Normal Kotodama generation and all 57 official sample mappings pass.

All five Apple slices and the ABI-25 package/resource checks pass. Swift host
compilation fails on two missing `try` expressions in fixtures; the prepared
repair is present in the incoming merge but requires native rerunning. The historical JavaScript suite records 3,634
passes and 15 failures in bundle accounting, manifest fields, scope and artifact
identity. Reviewed corrections retain strict parsing and unchanged bundle caps;
the reconciled manifest and bundle/replication proposals pass five and 35 focused
managed controls. Full integrated/native rerunning remains required.

Historical Android native arm64/x86_64 libraries and diagnostic APK packaging
pass source, native-payload, signature, manifest and six-method DEX census checks.
Dependency resolution recovered the original offline-cache packaging failure,
which remains retained. No physical tests have run; a connected Android device
has been requested. The diagnostic APK's debug signature is not release signing.

A genuine Core/Kagami finalized-execution fixture producer and public SDK bridge
consumer are prepared. Actual generation, consumer execution, current installed
packages, physical Apple/Android evidence and signed final artifacts remain open.
The candidate RAM interfaces and stable unavailable errors confer no encryption
qualification. Public guidance belongs in `iroha-docs`; this checkout's build and
validation do not depend on that sibling repository.

### ZK08 — current source contracts

Normal historical Core/Kagami, CLI, daemon and network-test builds pass. The
default Core selection passes 1,144 executions before the merge. All eight
separately optimized AXT controls complete natively, but source changes during
that run; its failed source guard prevents a successor union. The original
247-control mapping retains historical coverage; the replacement genuine producer
remains unexecuted.

The historical workspace all-targets check fails on six fixture/API diagnostics
in JavaScript host, network proofs and Torii tests. The incoming merge supersedes
some prepared corrections; the reconciled remainder needs a normal workspace
check. The panic-boundary inventory also detects new executor module roots that
require explicit review. Compiler artifact recovery never converts a failed
command into a pass.

Both historical four-validator component controls pass: transaction commitment
and lane transactions across a full network restart. Each observes exactly four
concurrent daemons. These controls do not qualify the merged source, finalized
FASTPQ authority, wallet workflows or release readiness.

The applied reconciled patch preserves incoming fixes and substantive assertions.
The unregistered initialization bank additionally binds successful STORE effects
to exact bytes in absolute cells and the same frame generation; eleven native
regressions remain pending. Frame lifecycle, permissions, initial history and
complete execution authority remain unfinished. Regenerate genuine fixtures and
run normal builds, native proofs, SDK consumers, formatting, codec guards,
workspace and four-validator checks on the recorded candidate. Retain all protocol
and resource limits; old native outcomes do not qualify changed source.

## Remaining execution sequence

1. Replace the insecure RAM-LFE encryption construction, retire diagnostic public
   surfaces, and complete its semantic relation and current SDK/consumer controls.
2. Rerun corrected FASTPQ fixtures and native regressions, then produce
   maximum ordinary/AXT proofs; qualify finalized-source network and hardware
   behavior without changing proof or resource limits.
3. Repair X509 terminal-claim privacy and the measured proving-time failure, with
   native parity, independent review and another complete maximum proof under
   unchanged coverage, byte, memory and time limits.
4. Complete IVM G3's native execution relation and finalized authority. Validate
   rejection of retired APIs and relation-confusion attempts in current consumers.
5. Capture one integrated source after the reviewed repair cohort; rebuild SDK artifacts,
   run applicable workspace checks and current four-validator tests. Preserve
   `3f + 1` committees, exact `n - f` certificates and no idle empty blocks.
6. Complete dedicated independent protocol/side-channel review and run the
   physical-device and authorized release-signing workflows. Each qualification
   claim requires its corresponding artifact-bound evidence.

## Evidence discipline

Normal builds, immutable executables and retained artifacts identify the tested
source. Distinguish a structural resource charge from measured process RSS,
a component proof from a complete credential, and local verification from
finalized network authority. Record failures and unexecuted checks explicitly.
Do not restore retired surfaces, bless changed source hashes, bypass native
provenance or relax proof/resource ceilings to obtain a passing result.
