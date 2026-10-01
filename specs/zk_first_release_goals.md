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

The following implementation boundaries require qualification against the final
unchanged candidate. Native measurements below are the last completed epoch9
validation. The coordinated successor includes reviewed source, fixture and
packaging repairs; those changes require fresh normal builds and tests. Earlier
source revisions and test totals do not qualify the current checkout.

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
relation. Reviewed memory-owner/request tests and an unregistered effective-address
bank are applied prerequisites. The epoch9 IVM quick selection passes 114 native
controls; a separate ownership fixture fails compilation on the retired
`Perm::empty()` API. Its successor repair requires native validation. These
prerequisites do not complete memory authority, the transition relation or
finalized State binding.

### ZK04 — FASTPQ

The public Quantity facade uses the masked bounded DEEP producer and independent
verifier. Transparent replay APIs require development/test features. Expected
statements derive from canonical authenticated context; proof consistency alone
does not authorize remote spending. RequiredMetal checks readiness before private
work and never silently falls back.

[FASTPQ readiness](fastpq_production_readiness.md) owns current relation and
resource limits. The integrated 77-query SHA3/SHAKE construction has authentic
native profile, seeded and ordinary/AXT single-proof outputs. Epoch9 passes all
four retained known-answer controls. The complete selected FASTPQ run records
1,298 passes and ten fixture failures, with no skips; reviewed resource, context,
and SHA3 fixture corrections preserve their substantive assertions and require
native reruns. Current maximum ordinary/AXT proofs remain unrun; earlier maximum
measurements cover the previous profile.

Dedicated independent derivations provide conditional soundness and ideal-QROM
hiding bounds for explicit query/attempt assumptions. They do not establish
concrete Keccak security, device behavior, side channels or finalized authority;
changed-source review admission remains required. Finalized-source four-validator
behavior, broader application shapes and hardware qualification remain open.

### ZK05 — ZK-X509

The MAIN relation retains all 49 registrations and supported certificate, CRL,
disclosure and ownership coverage. Paired FRI openings and RFC temporal geometry
derive a 9,420,938-byte maximum encoding against the unchanged 9,437,184-byte
ceiling. Geometry is 285 base / 280 auxiliary / 102 fixed columns, 1,681
constraints and degree four. It binds 72 authenticated times, 73 comparisons
and nonwrapping 38-bit differences. Private columns replay from clearing owners.

The last completed epoch9 normal optimized binary passes 147 focused native
executions covering 146 distinct tests, with no failures or ignored tests. These
include repaired padding/RFC channels, all 49 registration boundaries, bounded
DEEP/commitment parity and supported Metal controls. Genuine replacement profile
and component fixtures are integrated; superseded profiles still reject. Four
additional source-contract tests execute separately: two pass and two fail on
stale function/test-module boundaries. Their reviewed repairs preserve the
substantive ordering and closed-path checks.

The maximum structural credential produces a verified 9,420,938-byte proof.
Producer self-check, public verification and wrong-genesis/tampered-proof controls
pass. A separate native verifier process also accepts the retained bytes without
regeneration. Peak RSS is 9,983,410,176 bytes, below 12 GiB. Proving takes
2,405.926166 seconds against the unchanged 300-second target, so the maximum test
fails and activation remains unavailable. The retained public proof SHA-256 is
`8a58f948a3e57d29375fac0706012c891ae1f096f533042abb8039099276ab90`.
Evidence lives under
`dist/zk-remediation/2026-09-30/epoch9-core-privacy-fastpq-build4/privacy-native`,
with separate replay under `x509-epoch9-independent-replay1`. Both runs preserve
source and compiled-input guards. Concurrent load prevents a causal speedup
claim relative to earlier candidates.

The successor routes private quotient/native transforms through bounded exact-root
acceleration and reuses original native batches during initial sample/commit.
Successful entropy order, masks and proof framing are preserved by construction;
failed calls consume a prefix and stop under sticky accelerator quarantine.
Native parity, all original source/capacity controls and another complete proof
must validate these changes within unchanged byte, RSS and time limits.
Independent cryptographic, hardware and final-candidate qualification remain open.

### ZK07 — developer workflow

The Rust wallet owns clearing private inputs, canonical keys and circuit selection,
with typed preflight/proving errors. Callers supply actual notes and paths without
dummy inputs or transcript choices. JavaScript, Python, Swift, Kotlin/Java and C#
use the shared native owner with bounded inputs and asynchronous job custody.
Managed strings cannot promise erasure. Epoch9 normal Rust producers and the
57 official Kotodama sample mappings pass current authentic generation/checks.
The host bridge deadline repair compiles; Kotlin records 1,556 passes, Android
host consumers 291 passes, and two native host suites 62 passes against ABI-25.
Installed Python consumers pass five genuine maximum-tree wallet controls and
4,370 broader tests plus 281 subtests, with no skips. C# records 6,005 passes and
three typed multisig fixture failures; their reviewed repair requires rerunning
current consumers. Additional held Kotlin full-tree controls have six native
passes, but adoption still requires source-bound readmission.

All five Apple static slices compile normally. Packaging fails because three
maintained export inventories omit the existing coordinator install function;
the reviewed inventory correction retains exact-symbol and resource gates.
Swift host execution and package size admission remain pending. The successor
also corrects JavaScript artifact-identity fixtures. Compiler closure changes
require new authentic generation and consumer provenance. Physical devices and
signed multi-platform artifacts remain open; earlier SDK passes do not qualify
a changed candidate.

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

Epoch9 normal Core/Kagami, CLI, schema and four-validator test compilation pass.
The completed Core union records 1,146 native executions over 1,088 distinct
names, all passing with no skips; this includes the eight optimized AXT controls.
The separate IVM quick selection passes 114 controls. The complete execution
relations remain unavailable.

The normal workspace all-targets check fails with 34 rendered compiler diagnostic
spans across Core, Torii, JavaScript codec, IVM, deploy and Mochi fixtures/imports.
Reviewed corrections require a new normal check. Of 559 local compiler artifact
records, the original run admitted 506 and lacked prior compiler metadata for 53.
A separate recovery now authenticates exact historical compiler records for all
53 against historical, pre-check and retained bytes; normal current-source
readmission remains required and the failed workspace receipt is unchanged.

Two genuine four-validator runs fail at daemon startup because the embedded MCP
descriptor exceeds its unchanged 128-KiB byte cap. The reviewed correction
compacts only JSON whitespace, preserves all 60 descriptor records and adds
exact-boundary/rejection controls. Full four-validator Sumeragi, lanes, finalized
FASTPQ source and wallet workflows remain unqualified until successful reruns.

Preserve every substantive Halo2/note-STARK source assertion and X509 geometry
check. Source assertions and byte accounting do not replace normal native,
integrated workspace or four-validator network qualification.

## Remaining execution sequence

1. Replace the insecure RAM-LFE encryption construction, retire diagnostic public
   surfaces, and complete its semantic relation and current SDK/consumer controls.
2. Rerun corrected FASTPQ fixtures and native regressions, then produce
   maximum ordinary/AXT proofs; qualify finalized-source network and hardware
   behavior without changing proof or resource limits.
3. Resolve the measured X509 proving-time failure with native parity and another
   complete maximum proof under unchanged coverage, byte, memory and time limits.
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
