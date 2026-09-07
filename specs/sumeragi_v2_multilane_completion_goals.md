# Production multilane implementation goals

Set: 2026-09-06. Overall goal: **Active**. No milestone is complete.

Implement and qualify the Production Multilane Completion Plan as Iroha 3's
first release. The [closure ledger](sumeragi_v2_multilane_closure_ledger.md)
owns detailed invariants, production symbols, adversarial cases and release
gates. This record owns the current execution order and audit corrections.

## Design decisions

- One canonical implementation and explicit versioned Norito layout per
  capability. Reject obsolete layouts; no backwards compatibility, aliases,
  mixed consensus operation, implicit decoding or migration shims.
- Native participant controls certify routing and settlement. Economic effects
  enter WSV exactly once through the canonical global carrier.
- Reuse and validate the implemented participant predicate, durable claims,
  manifests and autonomous execution paths. Replace a design only for a
  demonstrated correctness or ownership gap.
- Kotlin `core-jvm` owns JVM models and protocol validation. Migrate every Java
  assertion, fixture and delivery consumer before deleting duplicate Java
  implementations; Java-source tests call Kotlin directly. Preserve JDK 8 API
  enforcement, defensive ownership and Android module separation.
- Keep formal models in the existing `formal/sumeragi_v2` owner. Public operator
  guidance belongs in optional sibling `iroha-docs`; it is not a build dependency.
- Use twelve lane-validator assignments across three independent four-validator
  dataspaces within an exact thirteen-validator global committee. `G-12P` keeps
  its existing identifier; twelve global voters cannot form a `3f + 1` committee.
- No new crates, manifest/lock changes, direct Serde dependencies, ABI version
  changes, production feature/environment switches or hardware-dependent results.
  Runtime bounds belong in `iroha_config` with deterministic defaults.

## Ordered milestones

| Goal | State / dependency | Owner | Completion criteria |
| --- | --- | --- | --- |
| M1 — Reconcile baseline and invariants | **Active** | Core/Sumeragi, model, formal and release tooling | Inventory current TODOs across source and root records; map each to implementation, negative tests and formal evidence or a source-proven exclusion. Reconcile stale ledger assertions, generated-source inventories and current runner paths. Audit the shared participant-role predicate across validation, Kura, State, recovery, diagnostics and retirement. |
| M2 — Native security and durable application | Open; M1 | Native AMX, model, Kura and State | Revalidate and repair `ML-NAT-01`–`ML-NAT-07` and `ML-KURA-01`: full signed source/session/route claims; exact incarnation and successor; atomic bounded groups; QC-bound manifest/proof; finality → sidecar/index → frontier publication; bounded startup repair, pruning, archive and index recovery. |
| M3 — Autonomous execution and ownership | Open; M2 | Queue, lane consensus, merge and canonical application | Close `ML-QUEUE-01` and `ML-AUT-01`–`ML-AUT-06`: signed strict admission, FIFO fsynced exact reservations, validator-independent checks, durable certified bundles, contiguous canonical merge sources, exact-base re-execution and post-commit Commit/ForgetCommit. Recover or release every ownership crash boundary without loss or duplication. |
| M4 — Automatic lifecycle and convergence | Open; M3 | Autoscale, lifecycle, Kura and diagnostics | Close `ML-LIFE-01`–`ML-LIFE-05`: atomic create/activate, every queue/reservation/certified/delayed/merge/Native blocker, one evidence-aware drain frontier, archive-before-removal, new-incarnation recreation and delayed-artifact rejection. Complete relevant owner/worker handoffs and qualify the recovered-payload application path. |
| M5 — Canonical clients and wire corpus | Open; audit may run alongside M2–M4, final parity follows settled wire | Rust/OpenAPI, Python, JavaScript, Swift and Kotlin/JVM owners | Close `ML-API-01`–`ML-API-04` and `ML-WIRE-01`: separate status/diagnostics, bounded State/Kura-derived conflict-aware rows, identical strict Native V2 accept sets and Rust-owned grouped positive/negative fixtures. Migrate JVM production/tests/runners to Kotlin ownership and rederive source/artifact inventories. |
| M6 — Formal and release qualification | Open; M1–M5 | Formal, integration, performance and release owners | Close `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, `G-SCALE`, `G-SDK` and `G-FINAL` on one source-bound candidate. Archive exact commands, outcomes, hashes, tools, configuration, seeds and hardware. Update current status and public docs only from fresh passing evidence. |

Every milestone stays open until its focused negative tests and matching formal
mutations pass. Interface properties use their explicitly defined differential
or static checks, not invented TLA+ proofs. Source presence, test collection,
old logs, skipped tests and mutable-tree inventories cannot close a release gate.

## First work queue from the current audit

1. Classify and resolve the owner/worker completion, recovered Broadcast rollover,
   composite planner queue-cut and live Ingress handoff TODOs recorded in the
   ledger. Include the post-WAL-append crash-boundary TODO in
   `v2_ready_durable_validate_adapter_preview.rs`. Do not remove a TODO merely
   because a neighboring path exists.
2. Add direct classifier/helper negatives for Prepare/Commit identity drift,
   predecessor mismatch, route/dataspace/incarnation/view/proposal drift,
   settlement tampering and a malformed later leg after an earlier route match.
   Compare the Core role classifier with model grouped-source alignment without
   weakening either validation boundary.
3. Requalify signed QueuePlan intent against stripped gossip, external ordinary
   execution, follower validation and historical replay. The old queue TODO was
   stale: signed intent and rejection paths already exist. Its replacement is
   a comment correction, not a new security implementation or test pass.
4. Migrate Sumeragi/Native Java consumer suites into Kotlin-owned testing and
   remove duplicate implementation only after all assertions and delivery paths
   move. Replace mutable public diagnostic model collections with owned values.
5. Repair Linux formal/chaos workflow invocation paths: the Ubuntu workflow
   currently requests `mktemp` under macOS-specific `/private/tmp`. Preserve
   private directory permissions and invocation/artifact isolation.
6. Rebuild fixture and recursive SDK source inventories from their canonical
   producers. Current fixture hashes and suite counts differ from ledger pins;
   copying new hashes into pins alone cannot establish correctness.
7. Seal a reviewed candidate only after source and receipt inventories match
   the canonical owners. The previous unmerged index entries are resolved;
   preserve unrelated work and recheck the index before candidate sealing.

## Required acceptance evidence

- Adversarial units cover drift/equivocation, stale incarnation/ABA, predecessor
  jumps, partial/duplicate/4,097-source groups, QC/manifest/proof/result forgery,
  eviction, malformed/oversized/symlinked storage, reservation duplication,
  base-state mismatch, bounded recovery and every persistence crash boundary.
- Current source-bound autoscale, Native application and autonomous carrier
  models pass TLC and Apalache; each mutation yields its expected counterexample.
  Retain the ledger's required deductive/production-trace obligations and hashes.
- Four-peer signed-RS16 suites cover automatic expansion/contraction, recovery,
  A/B/A same-ID recreation, grouped/mixed-role and same-route Native application,
  offline/Byzantine rotation, pruning, drain/archive and autonomous execution.
  Prove authenticated hold/drop acknowledgement before healing; skips fail.
- `G-12P`: 10/10 fresh deterministic seeds and a two-hour fault soak, grouped DvP
  plus autonomous work and lifecycle changes, full convergence, durable receipts,
  zero loss, zero rejected-after-acceptance transactions and zero duplicates.
- `G-SCALE`: pinned hardware, five paired one-lane/four-lane runs, at least 1.5×
  median committed throughput and at most 1.25× p95 latency at matched load.
  All queue, index, memory and disk bounds must hold.
- Before each Cargo invocation inspect `ps -axo pid,etime,command` and wait if
  Cargo/rustc is active. Never signal those processes. Use an isolated target,
  `--locked --offline` for build/test/lint, and a 20-minute build budget without
  treating timeout as success. Run focused crate/SDK suites and formal runners,
  then workspace build/tests, strict workspace all-target Clippy, `cargo fmt
  --all -- --check`, and the legacy-codec guard. Formatting does not resolve
  dependencies and does not accept Cargo build's lock/offline flags.
- No automatic release, deployment, signing bypass or fabricated pinned-hardware
  evidence. Keep unavailable external gates open and record their prerequisites.

## 2026-09-06 setup evidence

- Read the supplied plan, root status/roadmap and applicable repository rules;
  inspected Native, autonomous, lifecycle, client and formal source in parallel.
- Confirmed the old autonomous `execution_batch.is_none()` exclusion is absent
  and the canonical Native role predicate and V4 source claims already exist.
- `bash ci/check_sumeragi_v2_multilane_release_inventory.sh` exited 1: the SDK
  source-closure check rejected an existing unmerged Git index path. This is a
  current failed preflight, not a release or runtime result.
- Scoped `git diff --check`, local Markdown links, all six milestone entries
  and both root 300-line limits pass. Historical archive verification passes:
  64,736 records and 67,311 occurrences.
- `python3 scripts/formal/check_sumeragi_v2_multilane_models.py` exited 1.
  Reviewed Rust include inventories differ from current Kura/lane sources and
  State lacks a single stage-zero index entry. This is a structural failure;
  no TLC/Apalache execution or proof is implied.

## 2026-09-06 implementation checkpoint

- Kotlin's four remaining public diagnostics data classes are now immutable
  regular classes with explicit value equality. Private JSON serializers keep
  the exact wire fields/defaults; construction owns every list and nested JSON
  map/array, validates unsigned counters and rejects null list entries. Nesting
  and vector bounds apply before recursive copying. Existing copy-based test
  fixture calls now use explicit constructors.
- The isolated JDK-21 Gradle run of `:core-jvm:test`, filtered to
  `org.hyperledger.iroha.sdk.consensus.*` and
  `org.hyperledger.iroha.sdk.client.SumeragiHttpTransportContractTest`, passes
  **60 tests, zero failures/errors/skips** with JDK 8 API enforcement. This
  includes three new Java consumer cases against Kotlin, grouped Native
  fixtures, wire fixtures, endpoint separation and immutable ownership.
- Nightly/PR formal workflows and the standalone launcher canonicalize `/tmp`
  before creating private invocation roots. Four tests execute the actual
  setup blocks twice and pass; 27 formal-launcher tests pass. The complete
  proof-ledger test collection fails on an existing include-manifest count
  assertion, so no whole-ledger pass is claimed.
- Added six direct Native role/helper adversarial tests and one WAL crash
  regression covering both Prepare and Commit with two fresh reopens. The
  crash seam exists only under `cfg(test)`, after the fsynced append and before
  live Sign publication. Rust formatting passes on the changed test/source
  files. Rust execution and workspace formatting wait for existing Cargo/rustc
  jobs; no process was interrupted and no Rust test pass is claimed.
- M1–M6 and every release gate remain open. The later JVM ownership and
  delivery checkpoint below supersedes this checkpoint's migration state;
  formal engines, runtime corridors and scaling qualification remain outstanding.
- The legacy-codec guard and scoped whitespace checks pass. No new crate,
  dependency, manifest/lock change or compatibility path was introduced.

## 2026-09-06 JVM ownership and delivery checkpoint

- Migrated all six original Java suites (51 test methods and every assertion)
  into Kotlin `core-jvm`. Removed the five duplicate Java consensus classes,
  obsolete transport methods and unused operator configuration; private-settlement
  responder checks now invoke Kotlin's canonical BLS peer validator directly.
- All ten public wire vectors own immutable snapshots, including nested timeout
  signers and liveness vectors. Eleven new Java tests cover mutation, null entries,
  stable encoding/equality and 80 hostile count cases at ten decode boundaries.
  Allocation is bounded by the actual remaining compact-field prefixes. Native
  round constructors now enforce the parser's positive-height/u64-view domain;
  existing numeric-domain tests cover both constructor and parser boundaries.
- The current isolated JDK-21 selection passes **122 tests with zero failures,
  errors or skips**. The actual Java diagnostics runner passes its exact 59-test
  inventory, and grouped Java parity passes its six tests. These are local runner
  results, not sealed release evidence. The affected legacy Java build compiles
  and passes four selected settlement/configuration/read harnesses plus six
  onboarding tests after removal.
- Both JVM release legs now use Kotlin `:core-jvm:test`. SDK source closure
  removes the legacy Java/Norito-Java production/build roots and includes the
  migrated consumers and private diagnostics serializer. The external dependency
  schema now binds one Kotlin Gradle wrapper and rejects obsolete Java wrapper
  keys. Source-closure tests pass 23 cases; wrapper/queue schema controls and a
  current receipt smoke pass. Full receipt/bootstrap validation remains in progress.
- Formal include inventory and its per-root rejection controls pass 56 cases;
  17 loader, four candidate-builder and six Native source controls also pass.
  Current State/Kura admission and candidate-builder bindings now follow their
  actual ownership graph. Obsolete Native recovery rows, token translations and
  duplicate exemptions are removed. QueuePlan's 27-name exception set is now
  removed: 17 missing owner rows and five drifted rows are reconciled directly,
  retaining exact route-marker, terminal-conflict and pending-decode checks.
  Startup now declares its current replay-before-network path directly without
  token/name translations. The focused ledger/startup controls pass 85 cases.
- All milestones remain open. The new Rust classifier/WAL regressions still
  require focused execution, release-inventory registration and matching formal
  coverage. Other Cargo/rustc jobs continue to occupy the shared build queue.
- The current release-inventory preflight exits 1 because the closure ledger
  does not contain the current grouped Native fixture hash at its two required
  locations. Fresh Rust-owned regeneration remains a prerequisite; changing
  historical pins alone would not establish canonical fixture evidence.
- Receipt validation now requires the runner's canonical 12-step autonomous
  Apalache bound and explicitly rejects the old 10-step result. The production
  inventory remains 866 tests in 42 modules; its missing `queue::tests::` owner
  prefix was corrected with controls rejecting adjacent, unregistered prefixes.
- The final current-source structural formal preflight passes: five refinement
  kernels and the composed in-flight relation have their required source bindings.
  The corrected QueuePlan baseline check passes (one test); the earlier mutation
  batch loaded a pre-fix baseline and is not an all-pass result. No TLC, Apalache,
  deductive proof or runtime-network evidence is implied by these static checks.

## 2026-09-06 lifecycle and bounded-model checkpoint

- Retired the unlaunched-owner Certified-Serve completion API and its obsolete
  body-store writer/validator. Existing recovery tests now consume the real
  worker readback; corruption is checked separately before minting completion
  and after minting against the accepted payload store. Ten affected tests
  become eleven, preserving lease, registry, publication, restart and replay
  assertions. Direct Rust formatting and scoped whitespace checks pass;
  compilation and focused execution still await other Cargo/rustc jobs.
- Source inspection traced recovered Broadcast through authenticated output
  handoff and fsynced all-row owner retirement, exact Fetch queue cuts through
  the complete executor census, and the borrowed live ingress cursor through
  consuming launch. Their stale TODOs are corrected. The unused standalone
  production selector and duplicate queue-selection helpers are removed.
  Its existing test facade is now test-only and composes the live fenced-cut
  chain; all 65 selection assertions and production visibility are preserved.
  The older formal declarations now bind the live driver and reject the
  test-only facade as production evidence; seven focused controls pass.
- QueuePlan's exact source ledger and startup declarations pass 85 focused
  controls plus five production/semantic checks. The final structural model
  preflight passes after selector retirement.
  Certified-Serve source-contract rows now follow worker readback ownership;
  independent review of the stale whole-asset pin led to the four explicit
  contract repairs and derived-pin validation recorded below.
- Pinned TLC 1.7.4 passes all six finite model configurations. The production
  multilane mutation runner observes all 106 exact named counterexamples; the
  separate in-flight corpus passes its positive model and all 22 mutations.
  Summary logs and local model/config hashes are retained. An independent
  audit confirms the positive runs and runner-observed mutation results;
  raw mutation traces were deleted by those runners. Immutable trace evidence,
  Rust refinement proof and a sealed release receipt remain open.
- Pinned Apalache 0.52.2 typechecks all six models and passes the autoscale,
  Native evidence, autonomous carrier (12 steps), QueuePlan and Kura retention
  bounds. The in-flight 18-step check is still running. It must finish at its
  exact bound before any all-model Apalache pass is recorded.
- SDK inspection reproduced mutable nested participant settlement data after
  hash validation in JavaScript and Python. The repair preserves the checked
  settlement hash and wire JSON; grouped tests pass 62 JavaScript and 64 Python
  cases. Python constructors also own all three receipt vectors; 152 related
  Native/status tests pass. JavaScript declarations now reflect immutable
  receipt entries. Canonical runner/receipt/CI cardinalities are updated;
  23 source-closure and five receipt/schema controls pass. The broad
  receipt/bootstrap run finishes with 555 passes and one fail-closed cache
  parent-change rejection; that exact bootstrap case passes on isolated retry.
  The parent identity check remains enforced; a settled full run remains open.
- Swift diagnostics now checks integer number tokens before Foundation can
  normalize decimal/exponent notation. The existing grouped endpoint test
  covers those raw forms, Boolean confusion, overflow, exact `UInt64.max`
  and fractional quantity strings. Syntax parsing and the exact repository
  scanner/Foundation reproduction pass. Full SwiftPM testing still needs a
  real ABI-23 bridge artifact; the package gate is preserved and no stub used.
- Swift wire vectors now reject counts exceeding available length-prefix bytes
  before reserving storage; byte vectors validate their input range before
  copying. All 17 checked-in wire fixture XCTest methods pass in an isolated
  build of the exact wire/hash/network sources. The expanded malformed-input
  method covers ten vector boundaries, four byte-vector boundaries, platform
  overflow, empty-element prefixes and canonical empty liveness vectors.
  This standalone check does not substitute for the full Swift SDK gate.
- Swift swap metadata now validates TWAP with the canonical signed decimal decoder
  and rejects unknown metadata/tagged-value fields. The exact repository
  metadata and numeric declarations compile and pass five signed/zero/fractional
  positives plus 23 malformed-input cases in isolation. Matching assertions extend the
  existing grouped golden test; its full bridge-backed execution is pending.
  Metadata parity passes 65 grouped Python plus 152 related tests across both
  public decoders, 63 JavaScript grouped cases against source and rebuilt dist,
  and 123 focused Kotlin/JVM consensus/HTTP tests with zero failures/errors/skips.
  Kotlin uses the bounded Quantity codec; cohort source-closure/receipt checks pass 23/7.
- Registered the seven Native/WAL, two Serve and two previously omitted worker
  regressions. Five committed predicate changes and the 452-vs-453 seal baseline
  have source/history reviews; 57 Native and nine WAL controls pass. The grouped
  fixture hash still requires two fresh Rust-owned regenerations before repinning.
- Two additional canonical Kagemusha V1 share/bundle tests close an identified
  assertion gap in the prospective inventory: real BLS/Pasta positive fixtures,
  exact round/statement/signers, each Pasta equation, independent BLS failures
  and mandatory zero-top-up epoch-boundary seals. Direct formatting and source
  review pass; compilation and execution remain pending. The prospective named
  inventory is 879 tests across 43 module selections and 84 legs, with 463
  required regressions. Ten inventory controls, seven receipt controls and
  23 SDK source-closure controls pass; these are not Rust runtime pass counts.
- A full source-asset audit repaired four bounded contracts: follow the
  actual WAL-backed leader-wire consumer; retain relocated physical FIFO-order
  checks; authenticate missing paired Sign through its exact terminal ledger;
  retain recovered Fetch queue-cut retry semantics. Six worker fixtures now
  derive view transitions and protected Commit authority from authenticated WAL
  replay, preserving their assertions and the 1,024-view bound.
  All 54 source evaluations and five compaction tests with ten adverse controls
  pass; the revised worker fixtures still require compilation and execution.
- Three additional lane-work seals are reconciled after exact history/owner
  review, 19 public-certificate and ten historical-hydration adverse controls,
  and a passing five-owner complete-pin/direct-contract baseline. Two new Rust
  hydration tests and an extended public-finality regression await execution;
  full source integration remains independent of these focused results.
- All milestones and release gates remain open. The mutable shared source,
  fresh Rust-owned fixture regeneration, strict proof/receipt checks, SDK
  parity, four-peer networks, deterministic corridors, soak, scaling and full
  workspace validation still require qualification.

## 2026-09-07 recovery and formal evidence checkpoint

- Fixed canonical hydration's repeated-recovery capacity failure. Recovery now
  stages the complete current required set, validates every original conflicting
  quorum before eviction, preserves canonical order, and publishes only after
  exact retention. Fresh historical staging prevents completed-source resurrection;
  READY authorization follows successful installation. The existing ordinary test
  repeats actual persistence at capacity one; historical coverage includes a real
  certified-source completion before fresh hydration. Four cache regressions cover
  replacement/idempotence, protected evidence, preflight and atomic rejection.
  Direct formatting and source review pass; Rust execution remains pending.
- Registered those four cache tests in G-UNIT, preserving every prior row:
  526 focused tests, including 320 core. The current production inventory is
  881 tests across 43 modules and 84 legs, with 465 required regressions. These
  supersede earlier prospective counts and do not assert Rust execution.
- The exact formal preflight passes 55 controls, including 28 artifact-retention
  cases. Both freshly pinned TLC runners pass: 106 expected multilane
  counterexamples, one in-flight positive and 22 expected in-flight counterexamples.
  All 129 stderr streams are empty; retained private artifacts bind tools, exact
  input copies, argv, status, raw outputs, named traces and acceptance links.
  These are local bounded-model results. The original 18-step Apalache run is
  still active; no reduced bound or restarted run substitutes for its result.
- Reconcile the new hydration/cache and actual WAL authority source contracts,
  execute focused Rust regressions after the shared Cargo queue clears, and run
  full SwiftPM against the real ABI-23 bridge once built. The grouped fixture hash
  still needs two fresh Rust-owned regenerations. No milestone or release gate
  is complete.
