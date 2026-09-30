# ZK first-release completion goals

Set: 2026-09-26. Execution resumed: 2026-09-30. Overall status: **Active**.

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

The following implementation boundaries require qualification against the final
unchanged candidate. Earlier source revisions and test totals do not qualify the
current checkout.

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

`zk::verify_for_relation` admits only an explicit semantic role supported by its
compiled relation. IVM replay-binding circuits, registry support, keygen,
Torii derive/prove/jobs and associated SDK/CLI APIs are retired. Reserved names
reject. Production `IvmProved` admission stays closed until the complete native
transition relation and finalized State authority exist; G3 in the
[Kotodama/IVM goals](kotodama_ivm_completion.md) owns this work.

RAM-LFE registration, activation, restoration and receipts reject both signed and
proof BFV modes: the exact-lift profile loses public-key noise modulo 257. Public
evaluators refuse before private work. The execute API's false plaintext-opening
claim is removed. Secure encryption replacement, the complete relation and
integrated network qualification remain open.

The diagnostic interpreter owns clearing private tape cells and binds its
bounded BLAKE3 initializer, fixed-work modulo-257 reduction, canonical policy/tape
commitments and `initializer_descriptor_hash`. A trace is not a proof. The
[execution contract](ram_lfe_execution_proof.md), proposed
[semantic commitments](ram_lfe_semantic_commitments.md),
[replacement encryption](ram_lfe_encryption_replacement.md) and
[scalar packing](ram_lfe_plaintext_packing.md) own remaining construction and
resource obligations. Test-only planners and leaf circuits qualify no production
relation.

### ZK04 — FASTPQ

The public Quantity facade uses the masked bounded DEEP producer and independent
verifier. Transparent replay APIs require development/test features. Expected
statements derive from canonical authenticated context; proof consistency alone
does not authorize remote spending. RequiredMetal checks readiness before private
work and never silently falls back.

[FASTPQ readiness](fastpq_production_readiness.md) owns current relation and
resource limits and links complete two-child and maximum-occupancy measurements.
Independent hiding/soundness review, finalized-source network behavior, broader
application shapes and hardware qualification remain open.

### ZK05 — ZK-X509

The MAIN relation retains all 49 registrations and supported certificate, CRL,
disclosure and ownership coverage. Paired FRI openings and RFC temporal geometry
derive a 9,420,938-byte maximum encoding against the unchanged 9,437,184-byte
ceiling. Geometry is 285 base / 280 auxiliary / 102 fixed columns, 1,681
constraints and degree four. It binds 72 authenticated times, 73 comparisons
and nonwrapping 38-bit differences. Private columns replay from clearing owners.

The earlier optimized maximum proof fails `ConstraintOpening` during composition
after 3,368.579 seconds. Peak RSS is 9,639,247,872 bytes, within 12 GiB; no proof
is emitted. The September 30 repair cuts the SHA outer and carried word-product
recurrences at physical padding boundaries with the existing fixed selectors.
The resulting profile, updated component fixtures and all 49 maximum-source native
registrations pass their boundary controls: 2,831 edges, including mutation and
visitor checks. After the final helper cleanup, the repeated native boundary run
passes in 87.722 seconds with 5,895,077,888 bytes peak RSS. This is a boundary
test, not a complete proof. The complete maximum run reaches the final producer
self-check and fails `ProverSelfCheckFailed` after 6,241.900 seconds, with
10,295,918,592 bytes peak RSS. It emits no verified artifact; size compliance
is unestablished and the unchanged 300-second target is unmet. Five bounded
performance repairs and a test-only retained-public-candidate diagnostic are
prepared for native validation. Seven bounded replay/performance changes and
52 controls, including maximum fixed/OODS parity, await the next normal native
run. Complete successful proving, independent
verification and the unchanged runtime, memory and byte ceilings remain open.

### ZK07 — developer workflow

The Rust wallet owns clearing private inputs, canonical keys and circuit selection,
with typed preflight/proving errors. Callers supply actual notes and paths without
dummy inputs or transcript choices. JavaScript, Python, Swift, Kotlin/Java and C#
use the shared native owner with bounded inputs and asynchronous job custody.
Managed strings cannot promise erasure. Signed multi-platform artifacts and
current integrated qualification remain open.

RAM-FHE metadata requires the compiled initializer descriptor, exact seven-field
profile, sole encrypted-envelope mode and bounded unsigned dimensions. Associated
data uses canonical Norito independently of ambient flags. BFV secret keys own
clearing private coefficients and redact diagnostics. Public plaintext-encryption
helpers refuse with a stable unavailable error; deterministic encryptors belong
only to test fixtures. No retired mode, public seed overload or JSON compatibility
shim is admitted. Remaining scratch, current native consumers and integrated
SDK/Torii qualification must be verified on one candidate.

Public guidance belongs in `iroha-docs`; generated IVM help requires normal
pinned-source generation and authentic provenance.

### ZK08 — current source contracts

Preserve every substantive Halo2/note-STARK source assertion and X509 geometry
check. Source assertions and byte accounting do not replace normal native,
integrated workspace or four-validator network qualification.

## Remaining execution sequence

1. Replace the insecure RAM-LFE encryption construction, retire diagnostic public
   surfaces, and complete its semantic relation and current SDK/consumer controls.
2. Carry the passing maximum ordinary/AXT FASTPQ artifacts into finalized-source
   network and hardware qualification without changing proof or resource limits.
3. Pass the amended X509 focused suite on a normal optimized binary, then produce
   and measure the complete maximum credential; resolve any real relation or
   resource failure without dropping supported coverage or widening caps.
4. Complete IVM G3's native execution relation and finalized authority. Validate
   rejection of retired APIs and relation-confusion attempts in current consumers.
5. Capture one integrated source after concurrent merges; rebuild SDK artifacts,
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
