# ZK first-release completion goals

Set: 2026-09-26. Overall status: **Active**.

This record owns the remediation requested after the current-source ZK critique.
It supplements [first-release completion](first_release_completion_goals.md) and
[privacy closure](privacy_first_release_closure.md). Implementation remains in the
existing `optimizations` checkout. Separate SDK, Apple and network candidates preserve validation provenance; unrelated changes remain.

The outcome is useful, safe proof algorithms with simple developer APIs. Renaming,
disabling, status objects, source hashes or relaxed caps do not complete an algorithm.
External review and physical-device evidence cannot be self-issued.

## Goals and acceptance criteria

| ID | Status | Owner / outcome | Completion criteria |
| --- | --- | --- | --- |
| ZK01 | Complete (implementation) | Vega / secret-safe proving | Every private commitment uses public work dimensions and constant-time secret arithmetic; no raw uncleared witness copies cross the MSM boundary. Differential arithmetic, zero/partial/full row, failure/cleanup and worker-bound tests pass. Report resource limits only when actually enforced. Review other reachable secret hashing scratch, including ZK-ACE. Target timing and independent qualification remain ZK06. |
| ZK02 | Complete (implementation) | Confidential circuits / optional inputs | One owned note can be fully redeemed at maximum tree capacity. Absent inputs require no empty-leaf membership or caller-created dummy witness. Ownership, nonzero/duplicate-nullifier, range and conservation constraints remain enforced. Regenerate all changed circuit keys, digests and dependent fixtures; reject superseded keys. Positive and adversarial circuit/native/SDK tests pass. |
| ZK03 | In progress | Core / honest proof semantics | Generic verification cannot confer a stronger guarantee than its compiled relation. Retire IVM binding-only circuits, registrations, keygen, service routes and SDK/CLI consumers. Production IvmProved admission requires the complete native execution relation and authoritative finalized State anchor; replay or caller-supplied commitments cannot substitute. Implement the complete RAM-LFE program relation before enabling proof receipts. |
| ZK04 | In progress | FASTPQ / bounded private verification | Implement a sound source-state-bound relation with reviewed trace/composition masking and bounded verifier work; fit unchanged proof and total resource limits. Produce and verify real maximum-shape proofs, negative source/witness/statement cases, and CPU/accelerator parity. Full replay and unmasked offline compact proofs do not satisfy this goal. |
| ZK05 | In progress | ZK-X509 / complete bounded credential proof | Redesign or compose the full certificate, CRL, disclosure and ownership relation below 9,437,184 bytes without reducing supported coverage. Account for all segments, recursion, openings and prover resources together. Regenerate fixed profiles and produce actual positive/negative proofs before activation. Arithmetic size projections alone are insufficient. |
| ZK06 | Open | Cryptographic qualification | Obtain independent artifact-bound soundness, zero-knowledge, Fiat–Shamir/qROM, digest/multi-target, arithmetic and side-channel evidence for the selected release protocols. Keep explicit protocol-specific blockers; do not infer qualification from implementation markers. |
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

## Opening evidence

- Vega private row commitments trim by secret value and use variable-time MSM;
  the compiled Vega activation remains unavailable.
- Optional second confidential inputs require zero-leaf membership even when
  absent; a full 65,536-leaf tree has no such dummy leaf.
- `ivm-execution-v1` is a public-value equality circuit. Core correctness relies
  on unconditional deterministic execution replay.
- Admitted FASTPQ verification reconstructs complete batch commitments. Offline
  compact proofs are unmasked and cannot acquire production authority.
- X509's current maximum encoding is 19,156,074 bytes against a 9,437,184-byte
  ceiling. Even removing all P-256 trace-opening bytes leaves 10,452,074 bytes.
- The reviewed FASTPQ DEEP plan needs 20,199,768,064 bytes for its retained LDE
  alone; widened row openings occupy 616,448 bytes against a 524,288-byte target.
- The critique's focused Python selection passed 8 checks and failed 3 current
  source/inventory contracts. No Rust proof suite or timing attack was executed
  during that review.

## Current implementation and evidence

This section describes the September 29 source and scoped retained results.
The [exact preceding ledger](../docs/history/2026-09-29/zk-goal-ledger-before-current-source-reconciliation.md)
preserves the original observations and failures. Its earlier IVM proving routes,
CLI commands and network fixtures are historical; they are retired on current
main. No passing historical candidate qualifies the concurrently merged checkout.

### ZK01 — secret arithmetic

Vega private commitments use the fixed-window secret MSM, public dimensions and
clearing scalar owners. Twenty-five native arithmetic, cleanup and worker controls
pass. ZK-ACE borrowed private preimages and reachable hashing scratch also clear;
seven prover controls, 23 model controls and 71 primitive controls pass on their
recorded sources. Accelerator allocations clear after observed completion;
unknown completion quarantines their owners and blocks further private admission.
The unused memory-limit API was removed. Software defect remediation is complete;
compiler-created copies, device timing and independent review remain ZK06.

### ZK02 — optional confidential inputs

Absent inputs no longer require an empty-leaf path. One owned note can be fully
redeemed at the 65,536-leaf maximum, or redeemed with private change. Ownership,
range, conservation and nullifier constraints remain. Canonical replacement keys
and pins reject superseded keys. Actual native full/change proofs, adversarial
controls and Rust plus five supported SDK wallet workflows pass on retained
candidates. SDK artifacts also pass the captured Core decoder and typed verifier.
Same-candidate current network, distribution and device evidence remain open.

### ZK03 — complete proof semantics

`zk::verify_for_relation` admits only an explicit semantic role supported by its
compiled relation. Current main retires IVM replay-binding circuits, registry
support, keygen, Torii derive/prove/jobs and associated SDK/CLI APIs. Reserved
retired names reject. Production `IvmProved` admission stays closed until the
complete native IVM transition relation and finalized State authority exist;
its outstanding implementation is owned by G3 in the
[Kotodama/IVM goals](kotodama_ivm_completion.md). This hard cut does not complete
the execution algorithm.

Historical Torii/CLI/network results do not qualify current relation admission.
The shared integration proof fixtures and record/event/query callers now use the
public wallet facade to construct actual one-note full-unshield proofs. The
full-capacity wallet's wrong-role negative uses the retained confidential-transfer
key and schema. Native corruption, framing, proof-record, event and query
assertions remain; these migrated sources are formatted but not yet compiled.
The old successful IvmProved network case now requires exact unavailable-relation
rejection with unchanged state and independently applied progress. A current
20,543-file candidate retained 23 unfinished Sumeragi compile errors. A later
coherent Core/model/Torii build passes; two idle-smoke repairs await retry. The
native-lane correction passes 28 controls; the signed fixture now exposes manifest authority drift;
network qualification remains open ([record](../docs/history/2026-09-29/zk-current-network-qualification.md)).

RAM-LFE registration, activation, restoration and receipts now reject both signed
and proof BFV modes: the exact-lift profile loses its public-key noise modulo 257.
Public evaluators refuse before private work; 349 native crypto and 37 API controls pass.
An unrelated generic proof cannot satisfy execution. Secure encryption replacement,
complete relation and integrated network qualification remain open. The execute API's
false plaintext opening is removed; [boundary evidence](../docs/history/2026-09-29/ram-lfe-production-boundary.md)
records 63 schema controls; coherent reruns pass 367 Core, 396 model and 36 Torii controls.
Source-bound reruns supersede the historical 45/35 results with missing reused-dependency provenance.

The retained diagnostic RAM-LFE stage implements one bounded BLAKE3 initializer, fixed-work
modulo-257 reduction, distinct canonical policy/tape commitments and a mandatory
`initializer_descriptor_hash`. Tracing observes the sole eleven-operation
interpreter and owns clearing private cells; it is not a proof. The first normal
Cargo run passed 49 controls and failed three invalid BFV trace fixtures. Its
corrected normal rerun passes all 52 controls, with an independently reconstructed
1,024-byte XOF and fixed known answers. A subsequent source stage introduces one
validated shared clearing tape owner, bounded typed builder and sole private-frame
decoder, and fixes borrowed frame identities. Its normal native run passes all
61 controls and both API doctests with no source drift; the complete descriptor
and full-tape digest match independent known answers. Five configuration caller
controls also pass after correcting test access and diagnostic expectations.
The complete BFV semantic circuit and remaining
internal BFV scratch remain outstanding in the [execution contract](ram_lfe_execution_proof.md)
and [initializer record](../docs/history/2026-09-29/ram-lfe-bounded-initializer.md).
The proposed [semantic commitments](ram_lfe_semantic_commitments.md) replace private wire hashing;
[replacement encryption](ram_lfe_encryption_replacement.md) and [scalar packing](ram_lfe_plaintext_packing.md)
remain unqualified; the [test-only planner](../docs/history/2026-09-29/ram-lfe-structural-planner.md) passes nine isolated controls. The unused Pasta leaf passes
[18 native controls](../docs/history/2026-09-29/ram-lfe-pasta-leaf.md); its [circuit](../docs/history/2026-09-29/ram-lfe-pasta-circuit.md) passes ten isolated controls at maximum k=16 but misses the 20 ms budget (238.840 ms).
The distinct [BN254 naming cut](../docs/history/2026-09-29/bn254-poseidon-api.md) passes 19 native/four doc controls and actual GPU/AXT consumer compilation.

### ZK04 — FASTPQ

The public Quantity facade uses the masked bounded DEEP producer and independent
verifier. Transparent replay APIs require development/test features. Exact
statement, context and source bindings remain mandatory; proof consistency alone
does not authorize remote spending. Expected statements derive from the canonical
statement rather than caller-selected transcript internals. RequiredMetal checks
device readiness before private work and never silently falls back.

The source-lifetime repair transfers the physical trace owner and releases its
redundant 179,306,496-byte allocation. Complete shared Metal pool capacity is
charged through its lifetime. The repaired retained optimized binary constructs
and independently verifies both repeated-two-key, two-child fixtures:

| Route | Proof bytes | Construction plus self-check | Measured peak RSS |
| --- | ---: | ---: | ---: |
| Ordinary | 971,675 | 4,438.147 s | 1,881,849,856 B |
| AXT | 973,573 | 4,014.032 s | 1,875,820,544 B |

Both measured processes fit 2 GiB without changing caps. The prior ordinary run
exceeded that ceiling and remains in the historical record. Only `Cargo.lock`
drifted after the repaired capture; this evidence does not qualify current main.
Single-child CPU/Metal exact-byte parity and independent artifact verification
also pass on retained sources. These are contended observations, not throughput
measurements. Receipt:
`dist/zk-remediation/2026-09-29/fastpq-source-owner-proof-run1/two-child/complete-receipt.json`.

Maximum four-key ordinary and AXT proofs both pass unchanged limits: 973,336 and
1,010,229 bytes; construction/self-check 3,668.791 and 4,233.689 seconds;
peak RSS 1,856,045,056 and 1,875,656,704 bytes. Independent replay takes 7.70 and
26.87 seconds; 16 API controls pass. All 38 local dependencies rebuilt from the
frozen candidate. Exact receipts: `dist/zk-remediation/2026-09-29/fastpq-maximum-proof-run1`.
Independent hiding/soundness review, finalized-source network behavior and current
hardware qualification remain open in [FASTPQ readiness](fastpq_production_readiness.md).

### ZK05 — ZK-X509

The joined MAIN relation retains all 49 registrations and supported certificate,
CRL, disclosure and ownership coverage. Paired FRI openings and the corrected
RFC temporal relation derive a 9,420,938-byte maximum encoding against the
unchanged 9,437,184-byte ceiling. The current RFC geometry is
285 base / 280 auxiliary / 102 fixed columns, 1,681 constraints, degree four.
It binds the authenticated 72-time census, 73 comparisons and nonwrapping 38-bit
differences. Fixed matrices reuse bounded storage; private columns replay from
immutable clearing owners instead of retaining all masked coefficients.

Earlier retained controls and the failed `DerWitness` proof remain in dated records;
they do not qualify the later optimized candidate.
The corrected maximum temporal fixture and family-prefix storage pass 72 normal
optimized native controls, including all 285 base and 280 auxiliary columns on
ordinary and maximum witnesses. Maximum owned assembly is 200,816,874 bytes
against the unchanged 596,974,144-byte allowance. Replaced/partial owners clear;
nonzero omitted operands reject. Full-source and retained-binary guards pass.

The subsequent actual maximum proof fails at `BoundSources` with
`MainProofConstruction(TranscriptMismatch)` after DER/RFC binding completed.
No proof was emitted. Producer time is 1,062.696 seconds; measured peak RSS is
7,993,442,304 bytes. Metal executed 805 calls / 3,213 columns without fallback.
Base source sampling took 442.963 seconds and commitment 567.292 seconds;
no auxiliary masks, composition, DEEP or opening phases were reached.
The corrected RFC/SHA mapping passes nine native controls and the maximum handoff;
its first full BoundSources run overflows the normal stack. The heap-owner repair
passes all 13 clean native controls, including maximum BoundSources on the normal
stack and both independently verified component KATs. The fresh optimized run passes 109 controls;
its maximum proof fails `ConstraintOpening` during composition after 3,368.579 s; peak RSS 9,639,247,872 B fits 12 GiB, no proof is emitted ([record](../docs/history/2026-09-29/zk-x509-rfc-temporal-repair.md)).

### ZK07 — developer workflow

The Rust wallet owns clearing private inputs, selects the correct circuit and
canonical keys, and reports typed preflight/proving failures. Callers provide
actual notes and paths, with no dummy inputs or transcript choices. Low-level
caller-key builders are private; compile-fail controls enforce that boundary.
`change.into_input(index)` retains its intended owner. JavaScript, Python, Swift,
Kotlin/Java and C# use the shared native owner, with bounded inputs and accepted
asynchronous job lifetime. Retained native proofs and runnable public examples
pass; managed strings still cannot promise erasure, and signed multi-platform
artifacts/current integrated source remain unqualified.

RAM-FHE metadata now requires the compiled initializer descriptor. JavaScript,
Swift, Kotlin and C# use the exact seven-field profile, sole encrypted-envelope
mode and bounded unsigned dimensions; no retired mode or JSON compatibility
shim is retained. Kotlin passes 11 normal tests. Swift's normal five-slice artifact refresh and
nine profile/endpoint controls pass. C# passes 154 profile/identifier and two
endpoint controls under its pinned SDK; all 11 profile controls also pass without
a native-loader override. JavaScript's normal native rebuild and all 42 scoped
endpoint controls pass with no skips or source drift. Both JavaScript policy-list
routes now use the bounded lossless integer decoder, preserve raw `u64` values,
and reject duplicate fields and noncanonical profile numbers. Scoped lint and
code review pass. An old negative fixture assigned the existing policy ID; it
now uses a distinct valid ID and asserts that precondition. The original failed
run and the provenance guard's refusal before rebuilding remain recorded.
OpenAPI makes the descriptor mandatory; program associated data uses canonical
Norito independently of ambient flags. Python has no equivalent existing profile
endpoint, so no coverage is claimed for one.
The subsequent BFV backend-name hard cut and canonical outer/PRF transcript repair
pass 65 native controls and two doctests. Renamed tags pass 466 C# and two Python
controls; Kotlin compiles but missing Sumeragi fixtures prevent its current test
run. The preserved JavaScript candidate also passes its normal rebuild and 42
tag controls; Swift's refresh and integrated Torii remain open. See the
[transcript record](../docs/history/2026-09-29/ram-lfe-canonical-transcripts.md).
BFV secret keys now own clearing private coefficients and redact diagnostics;
confidential keysets now have an actual clearing destructor. Selected reachable
BFV/RAM scratch also clears. Retained runs pass 273 BFV and 86 targeted repair
controls (overlapping); a key-switch extension passes 49 focused controls.
Primitive state and diagnostic exports remain outside
this scoped [owner repair](../docs/history/2026-09-29/zk-key-owner-erasure.md).

Public Kotlin/Java, JavaScript and Swift plaintext-encryption helpers now refuse
with a stable unavailable error. Deterministic encryptors move to explicit test
fixtures and public seed overloads retire. Kotlin production/test compilation
passes; its JAR excludes diagnostic encryptors. Both complete selected Java
harnesses pass ([record](../docs/history/2026-09-29/java-canonical-bls-consumer.md)).
Kotlin runtime needs two canonical Kagami fixtures; the compiled producer fails at
block4's lane manifest authority check after the identity correction. No replacement bytes were invented.
JavaScript passes 148 build-tool controls; test arithmetic is absent from its package.
The repaired cold ABI-25 native build passes with all 69 local artifacts fresh;
all 47 endpoint controls pass, with strict types and lint. The first fixture failure remains recorded.
C# passes 269 managed controls plus four authenticated-native and three real wallet controls;
a strict derivation-revision mismatch is repaired. Current Swift five-slice builds are running.
Rust generic/specialized encryptors are private: 349 native and 37 API compile checks pass;
Core/model/Torii containment has passing evidence for all 799 selected controls; the corrected Torii fixture passes its 36-test rerun. Source-bound evidence is retained.
See [SDK boundary evidence](../docs/history/2026-09-29/ram-lfe-sdk-encryption-retirement.md).

Public guidance belongs in `iroha-docs`. FASTPQ plus 20 translations pass scoped
checks and its Rust example compiles. The RAM-LFE and fee-sponsor pages plus 40
translations state actual availability; content/i18n and RTL browser checks pass
([record](../docs/history/2026-09-29/ram-lfe-public-guidance.md)). Retired IVM help
requires normal pinned-source generation, never invented provenance.

### ZK08 — current source contracts

After the merge at `2478995058`, the token utility, two Halo2 source-inventory
guards and note-STARK constraint guard pass all 13 focused Python checks. The
separate X509 geometry selection passes nine checks. These validate the retained
source assertions and byte accounting; they do not replace the pending normal
native, integrated workspace or network runs.

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
6. Obtain independent protocol/side-channel review and physical-device/release
   evidence. These cannot be self-issued by implementation tests.

## Evidence discipline

Normal builds, immutable executables and retained artifacts identify the tested
source. Distinguish a structural resource charge from measured process RSS,
a component proof from a complete credential, and local verification from
finalized network authority. Record failures and unexecuted checks explicitly.
Do not restore retired surfaces, bless changed source hashes, bypass native
provenance or relax proof/resource ceilings to obtain a passing result.
