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

The merged base is `051df111` on `optimizations`, with the Kagami JSON fixture
correction. Its fixed-source normal build passes workspace all-targets checking,
Core/Kagami, Torii/bridge, CLI/daemon and both network-test targets. Genuine
finalized-execution capture, Kagami production, the public SDK bridge, schemas,
query/native fixture exports and executable examples pass. All 1,145 selected
Core controls, eight additional optimized AXT controls and both four-validator
component scenarios pass. The original
247-control mapping retains 246 exact names; the retired fixture control is
replaced by three genuine current producer/consumer controls. All 251 later
selected names are covered.

On that fixed source, ordinary controls pass 1,312 FASTPQ, 1,026 IVM-library, 84 RAM-LFE,
295 BFV, seven memory-request and 16 offline tests. The subsequent privacy repair
candidate completed normal production and optimized test builds and genuine
compiled-profile regeneration. Its complete 956-control selection has 942 passes
and 14 failures, with every selected name accounted for and unchanged source.
The profile gate, all 26 IVM memory controls, four source controls and 31 supporting
controls pass; the 894 ordinary X509 controls have 880 passes and 14 failures.
Failures identify a missing selected-input multiplicity in the closed P256 fixed
schedule, stale registration/resource expectations, terminal-link challenge-vector
over-allocation, and three protocol digest fixtures. Corrections and further source-join
work require the next normal build and native rerun. Historical proof receipts
and earlier SDK runs below qualify only their recorded source; no partial
selection closes a goal.

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
frame; complete replacement proof encoding and total working memory remain open. A separate fixed-basis native RNS key-switch component passes ten native tests,
nine complete public-fixture parity cases and 16 adversarial/public controls;
independent review verifies its public roots, primes and exact division. Its
3,031,040-byte owned allocation bound applies only to that component. Native
policy/key/input ownership, complete refresh, secure parameters and the full
program relation remain open; the component is not a qualified encryption
construction.

The recorded IVM library selection passes 1,026 controls, and all four selected
frame-initialization controls pass. The repaired memory-bank selection passes all
26 controls: seven initialization, ten LOAD and nine STORE. STORE fixtures now
capture the VM's initialized register values and preserve byte, log, gas, PC and
cycle assertions. The separate scalar/AIR
proof controls have no complete current-source result. Reviewed LOAD/STORE banks
remain unregistered prerequisites. Memory authority, initialization/history,
execution sequencing, integrated resource accounting and finalized State binding
remain open.

### ZK04 — FASTPQ

The public Quantity facade uses the masked bounded DEEP producer and independent
verifier. Expected statements derive from canonical authenticated context; proof
consistency alone does not authorize remote spending. Development replay APIs and
required-Metal readiness preserve their existing boundaries.

On the validated merged base, the library selection passes 1,312 ordinary controls,
and all 16 offline ordinary controls pass. A required-Metal maximum ordinary proof passes
production, self-verification and the native controls with unchanged source:
973,755 proof bytes and 1,561,083,904 bytes peak RSS. Its native production and
self-verification take 852.192443 seconds under concurrent work; this is not a
quiet timing comparison. The maximum AXT producer also passes on
unchanged source: 1,014,530 artifact bytes, 1,514,307,584 bytes peak RSS and
2,263.745643 seconds including self-verification. Concurrent work and a one-second
public stack sample preclude a quiet timing comparison. Both remain under the
unchanged byte, memory and work ceilings. Separate fresh verifier processes pass
both exact retained artifacts without reproving, with unchanged source and no
failed or skipped controls. Earlier source-drifted maximum runs remain failures
of source qualification.

Two actual-Metal controls on that source also pass scalar parity: all 63 continuation
prefix/body boundary combinations and leaf batches for all eight proof oracle
kinds. This covers the available Metal device, not the complete hardware matrix.

Historical seeded CPU and required-Metal proofs produce identical 485,219-byte
artifacts and pass positive/negative controls. Historical known-answer controls
and maximum artifacts do not qualify a changed candidate.

[FASTPQ readiness](fastpq_production_readiness.md) owns unchanged proof, payload
and work limits. Dedicated derivations provide conditional classical-ROM and
ideal-QROM bounds under explicit query/attempt assumptions. They do not establish
concrete Keccak security, full zero-knowledge, device/side-channel behavior or
finalized authority. Finalized-source network behavior, broader workload shapes
and current hardware coverage remain open.

### ZK05 — ZK-X509

MAIN retains all 49 registrations and the intended certificate, CRL, disclosure
and ownership coverage, but the complete credential relation is not yet proved.
Source review identifies missing verifier equations between P256/projection byte
declarations and the shared byte-I/O trace. The selected P256 tuple now has
80 fixed pair-to-limb events binding its bytes to actual initial arithmetic
writers and the digest-reduction input. The repair preserves existing widths and
degree bounds. Native comparison exposed the missing selected-input reads in
its closed fixed-schedule compiler; the compiler and boundary controls are being
repaired. RFC output metadata has six verifier-fixed field equalities; the next
candidate adds authenticated DER key-output reads. Real certificate/RFC/SHA/IO
value extraction and byte-source joins remain incomplete.

The private terminal repair removes 364 public scalars through 192 MAIN endpoint
equations. RFC retains 285 base / 280 auxiliary / 102 fixed columns and 1,702 local
degree-four constraints. Canonical quotient chunks use 137 independent Fp4 mask
coefficients with unchanged FRI caps. Revised codec arithmetic is 9,415,166 bytes
against the unchanged 9,437,184-byte ceiling. The current compiled profile was
generated and its independent pin passes natively. Subsequent fixed-schedule
and source-join changes require another generation and native proof run;
384 MAIN and 108 accumulator public scalars remain. Neither this partial repair nor local proof acceptance qualifies the
complete credential relation.

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

The remaining public terminal-product claims expose values not covered by trace
masking. Complete the numeric byte-source joins, terminal privacy, whole-transcript
hiding review and performance work before activation. A faster proof alone cannot
clear these independent blockers.

### ZK06 — cryptographic qualification

Dedicated independent source reads, derivations and adversarial controls are
recorded against exact protocol artifacts. Their scope is explicit: component
arithmetic, conditional soundness/hiding bounds and authentic native verification
do not establish complete soundness, zero-knowledge, transcript security,
side-channel resistance or release qualification. RAM-LFE construction and full
IVM semantics remain absent; X509 has missing byte-source joins, terminal-claim privacy and time blockers.
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
repair is present in the merged source but requires native rerunning. The historical
JavaScript suite records 3,634
passes and 15 failures in bundle accounting, manifest fields, scope and artifact
identity. Reviewed corrections retain strict parsing and unchanged bundle caps;
the reconciled manifest and bundle/replication proposals pass five and 35 focused
managed controls. Full integrated/native rerunning remains required.

Historical Android native arm64/x86_64 libraries and diagnostic APK packaging
pass source, native-payload, signature, manifest and six-method DEX census checks.
Dependency resolution recovered the original offline-cache packaging failure,
which remains retained. Physical testing requires rebuilt current packages and
a connected device. The diagnostic APK's debug signature is not release signing.

A genuine merged-base Core/Kagami finalized-execution fixture producer and public SDK
bridge consumer pass. Paired schema, query/native fixture generation and the
executable examples also pass. All 185 maintained golden-generation commands pass;
both sets of 61 outputs are byte-identical and match canonical files. The
top-up producer's two executions and eight consumer controls pass. Current installed
consumers and packages,
physical Apple/Android evidence and signed final artifacts remain open.
The candidate RAM interfaces and stable unavailable errors confer no encryption
qualification. Public guidance belongs in `iroha-docs`; this checkout's build and
validation do not depend on that sibling repository.

### ZK08 — current source contracts

The last validated merged-base build passes workspace all-targets checking and Core/Kagami,
Torii/bridge, CLI/daemon and both network-test targets. The default Core selection
passes 1,145 tests across 70 groups with no failures or ignored tests. The original
247-control map has 246 exact-name passes and three genuine producer/consumer
controls replacing its retired fixture control; all 251 later selected names
also pass. A separate normal optimized Core build and all eight AXT proof
controls pass on unchanged source, for a 1,153-execution union. The original
retained selection accounts for 1,152 executions; the extra current test covers
authenticated-genesis successor ordering and rejected-interval poisoning. The
earlier source-drifted run remains unqualified.

Both four-validator component controls pass on that unchanged source:
transaction commitment and lane transactions across a full network restart.
Each observes exactly four concurrent daemons. These are component scenarios;
finalized FASTPQ authority, wallet workflows and full release qualification
remain open. Compiler artifact recovery never converts a failed command into a
pass.

The current repair candidate changes privacy source, fixtures and profile commitments.
It requires normal compilation, genuine profile derivation and native reruns,
followed by current SDK consumers, formatting, codec guards and applicable
workspace/network qualification. The complete workspace test suite and strict
Clippy gate remain unexecuted on that final candidate. Preserve all protocol
and resource limits; previous native outcomes do not qualify changed source.

## Remaining execution sequence

1. Replace the insecure RAM-LFE encryption construction, retire diagnostic public
   surfaces, and complete its semantic relation and current SDK/consumer controls.
2. Preserve the clean FASTPQ maximum ordinary/AXT production and replay evidence;
   qualify finalized-source network and hardware behavior without changing proof
   or resource limits. Rerun affected proofs after any protocol/source changes.
3. Complete X509 verifier byte-source joins, repair terminal-claim privacy and the measured proving-time failure, with
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
