# ZK first-release completion goals

Set: 2026-09-26. Overall status: **Active**.

This record owns the remediation requested after the current-source ZK critique.
It supplements [first-release completion](first_release_completion_goals.md) and
[privacy closure](privacy_first_release_closure.md). Implementation remains in the
existing `optimizations` checkout. Separate captured SDK, Apple and network
candidates preserve validation provenance. Unrelated concurrent changes are
preserved.

The outcome is useful, safe proof algorithms with simple developer APIs. A
renamed placeholder, disabled feature, new status object, passing source hash,
or relaxed resource ceiling does not complete an algorithm. External review
and physical-device evidence cannot be replaced by self-issued certificates.

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
2. Make current relation guarantees explicit at the shared verifier and developer
   boundary; migrate callers and remove stale claims.
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

The earlier frozen Torii source builds normally and passes 126 of 129 selected
tests. Its three failures expose the retired service's `bytes_b64`/`bytes` output
mismatch. Do not restore those routes to make historical tests pass. The earlier
28-pass CLI and six-test network plan likewise require replacement with current
retained API/admission controls. Current generic proof identity and relation
verification still need same-candidate runtime qualification.

The shared integration proof fixtures and record/event/query callers now use the
public wallet facade to construct actual one-note full-unshield proofs. The
full-capacity wallet's wrong-role negative uses the retained confidential-transfer
key and schema. Native corruption, framing, proof-record, event and query
assertions remain; these migrated sources are formatted but not yet compiled.
The old successful IvmProved network case now requires exact unavailable-relation
rejection with unchanged state and independently applied progress. A current
20,543-file candidate captures these changes without drift. Its normal build fails
on 23 pre-existing unfinished Sumeragi migration errors; eight native controls
and six four-validator scenarios remain unexecuted. See the
[current network record](../docs/history/2026-09-29/zk-current-network-qualification.md).

RAM-LFE registration, activation, restoration and receipts now reject both signed
and proof BFV modes: the exact-lift profile loses its public-key noise modulo 257.
Public evaluators refuse before private work; 68 normal frozen crypto controls pass.
An unrelated generic proof cannot satisfy execution. Secure encryption replacement,
complete relation and current Core/Torii qualification remain open. The execute API's
false plaintext opening is removed; [boundary evidence](../docs/history/2026-09-29/ram-lfe-production-boundary.md)
records 63 schema controls and compiled-but-fixture-blocked Kotlin/Java tests.

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
The proposed [semantic commitments](ram_lfe_semantic_commitments.md) replace
costly private wire hashing and separate stable function identity from key rotation.
The unused pinned Pasta leaf passes [18 ordinary native controls](../docs/history/2026-09-29/ram-lfe-pasta-leaf.md);
the [unused circuit experiment](../docs/history/2026-09-29/ram-lfe-pasta-circuit.md) passes eight
isolated controls. Full relation and coherent production migration remain open.

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

Maximum four-key occupancy, maximum canonical AXT context and early over-limit
rejection are the next native proof controls. Their fixture migration must
preserve current `AxtSourceTransferOccurrenceV1` identity and exact transfer/claim
correspondence. Full hiding/soundness review, authoritative finalized-source
network behavior and current hardware qualification remain open. See the
[FASTPQ readiness contract](fastpq_production_readiness.md).

### ZK05 — ZK-X509

The joined MAIN relation retains all 49 registrations and supported certificate,
CRL, disclosure and ownership coverage. Paired FRI openings and the corrected
RFC temporal relation derive a 9,420,938-byte maximum encoding against the
unchanged 9,437,184-byte ceiling. The current RFC geometry is
285 base / 280 auxiliary / 102 fixed columns, 1,681 constraints, degree four.
It binds the authenticated 72-time census, 73 comparisons and nonwrapping 38-bit
differences. Fixed matrices reuse bounded storage; private columns replay from
immutable clearing owners instead of retaining all masked coefficients.

The pinned frozen debug binary builds normally with zero source drift. Maximum
whole assembly and profile controls pass (2); measured owned assembly is
542,564,850 bytes against a 596,974,144-byte allowance. Its 54,409,294-byte headroom
cannot alone admit the 64 MiB Metal pool. MAIN/ownership controls pass (85),
profile/codec/proof known answers pass (26), and typed policy/wallet/refusal
controls pass (36). These counts describe separate selections on one immutable
binary, not a current workspace result.

RFC controls pass 53 and fail four stale test fixtures/expectations. The reviewed
two-file amendment corrects the actual maximum temporal-census fixture, degree
histogram, non-padding capacity and physical-copy row; it changes no runtime
relation, profile or cap. Its fresh normal optimized build and controls precede
the next maximum proof. The prior complete attempt returned `DerWitness` after
1,159.68 seconds and produced no proof. No complete maximum proof currently
passes the unchanged 300-second/12-GiB limits. Activation remains unavailable;
independent review and real positive/adversarial proof evidence are required.
Details: [September 29 X509 record](../docs/history/2026-09-29/zk-x509-rfc-temporal-repair.md).

A subsequent storage correction retains public-family field prefixes
and reconstructs unchanged full RFC rows, rejecting any omitted nonzero operand.
Old/new allocation overlap and surviving schedule capacity are checked, and
replaced/partial private owners clear. Eight exact-source isolated owner controls
pass. Isolated actual ordinary/maximum fixtures pass all 285 base and 280 auxiliary
column checks, temporal adversaries and generic field-extension controls without
source drift. Maximum retained RFC payload falls from 421,806,624 to 80,058,504
bytes. Normal Core, whole-assembly and complete-proof qualification remain pending
in the [family-storage record](../docs/history/2026-09-29/zk-x509-family-storage.md).

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

Public guidance belongs in `iroha-docs`. The FASTPQ guide and 20 translations
pass scoped content/i18n checks and its Rust example compiles. Retired IVM help
snippets must be regenerated through the documentation repository's normal clean,
pinned-source workflow; manual generated edits or invented provenance are invalid.

### ZK08 — current source contracts

After the merge at `2478995058`, the token utility, two Halo2 source-inventory
guards and note-STARK constraint guard pass all 13 focused Python checks. The
separate X509 geometry selection passes nine checks. These validate the retained
source assertions and byte accounting; they do not replace the pending normal
native, integrated workspace or network runs.

## Remaining execution sequence

1. Replace the insecure RAM-LFE encryption construction, retire diagnostic public
   surfaces, and complete its semantic relation and current SDK/consumer controls.
2. Produce maximum-occupancy ordinary/AXT FASTPQ artifacts through current facade
   fixtures and independently verify them under unchanged resource limits.
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
