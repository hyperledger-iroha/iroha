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
  bounds. The original in-flight 18-step check later ended without a terminal
  result; that attempt does not qualify its bound.
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
  These are local bounded-model results. The original 18-step Apalache run has
  no terminal result or live process at the latest inspection; its retained output
  ends at state 13. A separately retained fresh 18-step attempt has started and
  passed typechecking; its bounded result remains pending. It does not replace
  the original incomplete attempt.
- Reconcile the new hydration/cache and actual WAL authority source contracts,
  execute focused Rust regressions after the shared Cargo queue clears, and run
  full SwiftPM against the real ABI-23 bridge once built. The grouped fixture hash
  still needs two fresh Rust-owned regenerations. No milestone or release gate
  is complete.
- Recovery-owner source qualification now passes 84 checks on Python 3.12.14
  with the repository-pinned pytest 9.0.3, plus the five complete owner seals.
  The persistent recovery-cut component passes 114 checks on the same supported
  environment, including full component integration and factory shadow/rebinding
  negatives. These replace older-interpreter diagnostics as the current local
  source evidence; they do not prove Rust behavior or full release closure.

## Terminal autonomous replay checkpoint

- Full vote/QC and exact executable READY authentication precedes terminal
  duplicate results, including the complete-certificate historical shortcut.
  Historical payload lookup attaches the exact public hint before accepting own
  application or the applied predecessor; an advanced frontier alone is insufficient.
- A bounded read-only inventory resolves exact applied cache slots. Shared quorum
  preflight rejects conflicts before retiring sessions/locks or READY references;
  unrelated owners, capacity and separate output handoffs are preserved. Cleanup
  precedes fresh signing and guarded persistence hydration.
- Five added cache regressions bring the current G-UNIT registration to 531
  (325 core), preserving all 526 prior rows. The real merge fixture now exercises
  actual pre-application cache owners, full output
  capacity, observer/member replays, malformed signatures and incorrect READY roles.
  Rust execution remains pending. The terminal independent source loader and
  62 rehashed semantic mutants pass as a 63-case cohort. These include autonomous
  role scoping, exact READY cryptography/payload validation, full-slot conversion,
  output preservation and retirement ordering. A fresh dedicated baseline also
  passes after the test extension. The broader exact-output source check failed
  with 77 errors; full proof-ledger qualification remains open.
- The test extension now follows the actual first merge with source height 4,
  lane height 2, and economic merge height 5. Context 6 checks non-genesis own
  application and the older lane-height-1 receipt after frontier advancement;
  distinct descriptor/incarnation identities are rejected. All voting adapters
  retain the same Kura-local key while global leaders rotate. Original first-cycle
  assertions remain. Shared helpers retain the exact FIFO pipeline and derive
  later parents/contexts from canonical State/Kura. Rust execution is pending.
- Two obsolete per-proposal hydration clauses in the broad checker now defer to
  its existing canonical atomic-batch contract. A copied-source positive baseline
  and rehashed reverse-before-batch negative pass, and the full checker retains
  exactly one call to that owner. A fresh broader run is pending; remaining
  source errors are not waived or repaired by copying new digest values.
- The 531/881 approval operation identities are current, with no compatibility
  aliases; all 74 other ordered operations remain unchanged. Three approval
  controls and ten independently omitted-result receipt fixtures pass on the
  supported Python runtime. The final selected inventory cohort passes 84 checks,
  including exact constants and name/count/feature/prose adversaries; these do
  not execute the registered Rust tests.
- The independent recovery/capacity contract is reconciled to three current
  storage owners and passes 33 tests plus ten new subcases with copied-source
  positive baselines. One initially surviving role mutation led to a stronger
  pre-recovery ordering check; the final full cohort passes. No model or Rust
  runtime result is inferred.

## 2026-09-07 host restart and resumed qualification

- A host restart cleared `/private/tmp`, including the temporary Python runtime,
  raw formal artifacts, source-check logs and the shared unfinished Core/Apple
  builds. Both active checker handles are missing. The second 18-step Apalache
  attempt and the broader source rerun ended without a retained terminal result;
  neither is a pass. Earlier reported observations remain historical results,
  but their deleted raw artifacts cannot supply current release evidence.
- The nine terminal production/test files still match every hash in the last
  observed stable manifest. A new manifest and the restored Python 3.12.14 runtime
  with exact script dependency pins live under ignored
  `dist/multilane-validation-20260907/`. New validation records use that durable
  directory. No Cargo process was started or interrupted by this task.
- The ingress source seal was traced to its exact historical preimage. Current
  ownership fail-stop, finalized proposal/body checks and restart admission are
  now explicit ordered contracts. Six independently rehashed guard-removal or
  inversion cases pass with positive copied-source baselines. The complete
  terminal cohort then passes all 69 cases against the revised seal: 68
  rehashed negatives and the independent canonical baseline. Rust execution, the
  full source gate and all milestone/release qualification remain open.
- Two merge-cache seals now follow exact historical-owner review, 21 passing
  copied-source baselines and 21 rehashed semantic negatives. The final contract
  passes all eight cache owners. It binds authenticated persisted-candidate reuse
  to the complete round, parent, QC and durable signing bytes before reconstruction.
- A further Queue audit found public raw-key release methods with only unit-test
  callers. They are now crate-local test fixtures, including the single-key journal
  leaf. Shipping direct release requires move-only strict-absence authority at the
  shared batch sink; its fixture variant is absent from production. The existing
  real-planner regression adds missing, incomplete and identity-mismatched authority
  cases with unchanged journal bytes, FIFO, owners and startup gate before the
  original successful recovery. Registered Rust test counts are unchanged. Direct
  formatting and independent source review pass. Two Python tests pass the canonical
  baseline and all 14 rehashed negatives; the complete QueuePlan contract and both
  revised production bindings also pass. All 208 original assertions remain, with
  ten added assertions in the existing fixture. Rust execution is pending. The
  three additional Rust files have a durable ready manifest. The current four
  retired-codec/compatibility guards pass.
- The composed model audit found that its fixed configuration omits the defined
  `MLTerminalDispositionExclusive` invariant. It also permits direct release while
  Kura custody remains active; subsequent Commit/application guards do not check
  that disposition. Pinned TLC confirms a guided trace conjoining every edge with
  the original `Next`: active-Kura/direct release at four actions, FIFO plus lane
  Commit at 19, and FIFO plus WSV application at 20. All 18 configured invariants
  and the omitted exclusivity predicate still accept that complete 20-action
  trace. The shared Rust state predicate likewise permits the inconsistent state.
  Matching model/kernel guards, stronger configured invariants and counterexample
  controls are being implemented. This is a verified abstraction/refinement gap,
  not evidence of a live-network exploit; the old 18-step run cannot qualify it.
- The complete exact-output source diagnostic finishes with 71 errors in 936.75
  seconds. All failures and before/after input hashes are retained in
  `dist/multilane-validation-20260907/exact-output-result.json`. Four input files
  changed during the run, so it is mutable-development evidence. Ordinary ingress,
  lifecycle completion, worker ownership/ACK and finalization source contracts
  remain under review; the focused passes above do not waive these failures.

- The combined repair is now source-ready: authenticated Kura activation adds only
  its actor to the payload binding; every other transition preserves that binding.
  Direct actor-free release requires absent Kura custody, and release dispositions
  exclude lane Commit and WSV application. Retired replica proof stutter remains
  valid. The terminal extractor checks the complete state predicate. Existing
  Rust fixtures derive custody from real certificates rather than all-member masks;
  480 refinement and 125 Kura assertions remain, with 17 and three added assertions.
- Fresh exhaustive TLC passes all 20 invariants over 280,818 distinct states, depth
  36. All three new mutations fail at their named invariant after 4/19/4 actions.
  Four explicit original-`Next` replica and release paths also pass. A complete
  fixed-plus-25-control rerun is pending after one earlier concurrent TLC process
  failed during standard-module extraction; that failed attempt is retained.
  Current-model Apalache, Verus and Rust execution remain unqualified.
- The combined ready manifest verifies 17 Rust and five model/configuration files
  against their separate owner records. Its latest Rust write is
  `2026-09-07T03:52:05.610100Z`; the shared Core owner has it for capture. The
  post-custody terminal source baseline passes. These focused results do not close
  the 71-error full source diagnostic or any milestone/release gate.

- The fresh private-temp fixed-plus-25 TLC matrix passes after independent result
  inspection: the positive explores 280,818 distinct states at depth 36 in 22.02
  seconds; every mutant returns exit 12 and its exact named invariant. All 22
  original configuration bytes and their order remain intact. The prior extraction
  failure remains retained separately. Raw logs, tool/source hashes, commands and
  independent verification are under `formal/direct-release-repair/tlc-current-private/`
  in the durable validation directory. Four explicit replica/release positives
  pass at 25, 10, 10 and 11 actions. Current-model Apalache has a separate run;
  Verus and the planned 89 direct-binary Core regressions remain pending.

- The final repaired-model registration selection passes 54 checks in 459.49
  seconds, including complete production acceptance, semantic custody/release
  controls, exact runner registration and the existing ledger-count negative.
  Nine source/doc inputs remain unchanged across the run. All 103 prior test/helper
  ASTs and the original 22 mutation mappings remain; no existing SHA pin changed.
- Ordinary ingress now follows its exact historical preimage and authenticated
  historical-body worker handoff. All 24 copied-source baselines and 24 rehashed
  negatives pass before the two owner seals are updated; the final independent
  owner check then reports zero errors. Registration counts are unchanged. All
  22 combined runtime/model inputs remain identical. The separate decided-lane
  `commit_certified_serve` contract and configuration seal remain under review.
  The broader 71-error result predates these corrections and is not a current
  failure count or a full source pass.
- The pinned Verus binary and cargo-verus match their expected Darwin arm64
  digests. Side-by-side Rust 1.95.0 restores the exact version probe without
  changing repository/default toolchains. Its bounded Cargo proof remains queued.
  A public consensus documentation draft is retained in the validation directory;
  application to `iroha-docs` and all 20 translations awaits matching evidence.

- A read-only scaling readiness audit identifies an outstanding implementation
  outcome: the checked-in G-SCALE runner expects an external trial executable,
  while `tx_load.py` currently exposes aggregate submission/commit estimates and
  the CLI ping batch discards per-transaction confirmation results. Implement a
  real collector using the existing deployment/load paths, with exact transaction
  identity, offer/admission/commit timing and resource observations, before the
  five paired measurements. The validator's fixture samples cannot close this
  gap. Source hashes and the bounded review are retained in
  `dist/multilane-validation-20260907/scaling-collector-readiness.json`.

- Configuration geometry reconciliation passes the complete current Root owner
  and 24 independently rehashed semantic/order controls before and after its one
  reviewed seal update. All 12 existing test registrations and four assertions
  remain. Decided-body serving passes 72 copied-source baselines and 72 rehashed
  negatives before adding its 19 reviewed owner seals; both canonical and existing
  fixture checks then report zero errors. No runtime source changed in either
  cohort. Raw preimages, diffs, mutants, commands and results are retained under
  `config-geometry-review/` and `decided-serve-review/` in the durable validation
  directory. Broader source, Rust and release qualification remain open.
- The shared Core build captured all 22 combined inputs at
  `2026-09-07T04:28:27.890002Z` and passed in 564.617 seconds. All 17 Rust inputs
  matched before/after/current; all 22 matched capture/current. The command was
  locked but not offline, and unrelated SoraFS Node files changed during the
  build outside its artifact dependency graph. This is scoped Core evidence,
  not the prescribed immutable locked/offline final gate.
- The first source-matched 89-test direct run finished at `04:50:50Z`: **39 pass,
  50 fail**, no skips, unchanged selected sources and copied binary. The separate
  real-wrapper regression also fails. The production wrapper minted the former
  model SHA while its shared authentication kernel required the repaired SHA;
  pure refinement tests passed while otherwise-valid runtime transitions failed.
  Correcting all four source-identity words is necessary; no gate is weakened.
- Four runtime correction files are ready at `05:01:04Z`, bound by
  `core-runtime-failure-review/runtime-ready-manifest.json`. The existing wrapper
  test now covers all 25 producer positions across 1/4/7/13-member committees,
  rejects stale source/post-state witnesses and hashes the actual TLA source.
  Its 661 prior assertions remain, with three added assertions. Three application
  fixtures use matching production network context; three Queue initializers use
  the committed manifest helper, retaining all 218 assertions. Kura's shared
  fixture now uses four nonzero deterministic BLS seeds and retains 229 assertions.
  A new shared Core build and all 89 regressions plus the wrapper test are pending.
- Lifecycle source reconciliation passes 46/46 checks after preserving every
  original assertion and correcting actual split-owner fixture targets. Four
  lane-output owners pass all 43 independently rehashed controls before and after
  exactly four reviewed seal changes; all prior registrations/assertions remain.
  These results do not close the remaining full source diagnostic.
- The queued Verus harness finishes naturally at `04:57:42Z`, exit zero, with
  1,690 and 221 verified/zero errors in 57.461 seconds. Its unchanged evidence
  driver correctly refuses qualification: the historical second count was 172,
  and `v2_core/tests.rs` changed during the run within its 48-input validation
  inventory. The actual proof census/input closure is under review; retain the
  initial driver and rerun on a fresh stable scope after the shared Cargo slot.
  Both 18-step Apalache runs remain live; the old model cannot qualify the repair.
- The second shared build stops after 474.53 seconds on separate SoraFS/Torii
  compile errors, with no new Core test artifact. The subsequent source audit
  finds a 28th action already in TLA/shared Rust but absent from the 27-entry
  trace mapping and the production witness wrapper. The new wrapper and regression
  preserve every prior assertion. Structural dispatch/proof extraction, all 28
  mappings, two original full trace baselines and 160 independent rehashed
  controls pass; Rust/math sources remain frozen for execution.
- The third shared build finishes at exit 101 after 323.212 seconds. Only the
  separate Torii library-test target fails on relocated include paths and
  following type errors. Fresh Core and SDK test artifacts are emitted with
  zero captured source drift. Review verifies all 26 ready hashes/mtimes and
  all 21 Rust before/after/current hashes. The exact 91-test Core run is in
  progress. All 24 SDK status tests pass, including the async global wrapper;
  source/binary hashes remain unchanged. This does not qualify the combined build.
- The strict scaling schema requires complete fixed-schedule transaction traces,
  fully drained warmup and bounded drain-tail accounting. Its combined runner and
  validator selection passes 69 tests and 95 subcases, retaining all original
  assertions. The production collector is being implemented privately during
  the build freeze; actual routing/resource observations and trials remain open.
- Runner ownership reconciliation passes 27 rehashed semantic negatives with
  copied positives before ten live seal updates and two new helper pins; all
  28 focused tests pass afterward. Generation cancellation, receipt flushing,
  current archive recovery and finalization/error ordering are retained. Ingress
  effects separately pass 81 copied positives and 81 rehashed negatives. The
  full source diagnostic and release gates remain open.

### 2026-09-08 resumed validation and merge reconciliation

- The third-build copied Core binary completed all 91 exact regressions at
  `2026-09-07T06:03:36Z`: **74 passed, 17 failed**, with every captured source
  and the copied binary unchanged. The 24 SDK status tests passed separately.
  Retained raw results are under `core-direct-91-retry3-matched/` and
  `sdk-status-direct-24-retry3/`; these are pre-merge results, not current-source
  or full-build qualification.
- Three failures expose invalid fixture setup: two voting journals were opened
  before final lane context installation, and historical READY recovery lacked
  the carrier's signed finality proof. Fourteen application failures split into
  missing executed results, invalid reservation authority, an undersized startup
  route budget and a fabricated zero-length hash-only block record. Incoming
  source already corrects several of these boundaries; reconcile exact functions
  before adding changes and retain the original rejection/ownership assertions.
- The incoming merge changes 8 of the 19 actual Verus compiler inputs and 12 of
  the 48 wider proof inputs. The reviewed pre-merge 221-obligation census and
  previous source seals cannot qualify those changes. The next Verus launch is
  held for source and census review. The two existing Apalache processes remain
  live as directly observed on September 8; neither has a terminal result.
- Shared Rust and source-checker conflict resolution belongs to the merge task.
  No multilane Cargo process is launched while those inputs are unresolved.
  Current SDK/CLI changes also require fresh provenance and execution. The
  private transaction collector includes fixed-slot completion ordering and
  exact deadline tests; no private test is reported as an executed Rust test.
- After merge release, the collector and one final-context voting-journal
  regression are published. The frozen V4 manifest contains 51 files, including
  45 Rust sources. The successor selection explicitly accounts for all 91 prior
  Core cases and adds five required cases, for 96; SDK status retains 24 and CLI
  collection adds 16. Formatting, diff checks and retired-codec guards pass.
  Compilation and execution are still pending. Complete owner-published Kura
  resource accounting is being implemented privately; incomplete inventory must
  remain unavailable to the scaling collector.

### 2026-09-08 sixth-build focused regression result

- The fourth common build emitted Core/SDK artifacts but failed CLI compilation.
  The CLI repair uses explicit expression grouping in the native Norito JSON
  macro and the canonical metadata JSON conversion. The fifth build passed;
  its exact Core selection finished 95/96, SDK status 24/24 and CLI 13/16.
  All original failures and source receipts remain retained.
- The sixth common build passes after two scoped test corrections. The READY
  recovery fixture restores the exact canonical authority only after asserting
  durable PoP recovery under the intentionally empty authority map. It retains
  all 26 original assertions and adds three checks. The virtual clock yields
  before advancing future time and rechecks newly registered waiters; all 93
  prior assertions remain and eight timer checks are added.
- The source-matched Core binary passes **96/96**, zero failures/ignored, from
  `01:52:17.947197Z` to `02:05:28.643058Z`. CLI passes **17/17**. SDK retains
  **24/24** on its exact unchanged fifth-build binary, validated again by the
  successful sixth build and unchanged SDK source closure. All 51 selected
  sources and each private binary remain unchanged throughout their runs.
- V6 manifest SHA-256 is
  `f38a945e1895532d5ce95990131d2637a8b5367da00556a2cacfb4302dfa46ad`;
  Core selection V5 is
  `f8a5b0b59484409033d2823840e225f0a77c4e7ef112b6ffe825c959504e5ae9`;
  CLI selection V2 is
  `9d75d4e4af274e06d0b8246de47af57c7d6f4ccd776c5cdc8f6eaee868a210d7`.
  Core binary SHA-256 is
  `f179fa1c6ffdc682fc0ca536fb08522dd8f9377c46bb953a518149991224d234`;
  CLI is `24d69ce37ffeb81a18591d4d696dccc29a2f46607594674ec78a26bf9eee5fa7`;
  SDK is `3d5c90e4927abb905d8b3c3c73cea79ba0bbbcc4d3a84d1066e3f681fc5a6521`.
  The retained `core-sdk-cli-retry6-provenance-review/` receipts bind the
  compiler outputs and current source to these direct test runs.
- Fresh-target Verus reports 1,690 library and 221 project queries with zero
  errors and exactly 19 compiler inputs. Its strict driver remains unqualified
  because the broader checkout digest changed during execution; zero declared
  input changes do not override that condition. The earlier source diagnostic's
  41 errors and unrelated input drift remain retained. The corrected sealed-source
  diagnostic below passes; no new proof run qualifies those changes.
- Complete Kura resource accounting, exact workload routing, SDK parity,
  four-peer/13-global-validator network runs, ten deterministic seeds, the
  two-hour soak, five pinned scaling pairs and final workspace qualification
  remain open. The focused passes close no release gate.

### 2026-09-08 disk-accounting writer regression result

- Code review found ten identical total-cache deltas immediately after the
  enforced-cache helper, which already updates both caches. The scoped runtime
  correction removes only those duplicate calls. Legitimate total-only DA
  deltas and all mutation guards remain unchanged. Two new actual-writer tests
  and eight raw-cache checkpoints inspect actual file lengths before any refresh
  could conceal duplicate accounting. Original assertions remain intact.
- The seventh common build passed. Its five exact writer tests finished three
  pass/two fail, with unchanged selected sources and binary. The process fixture
  had no configured lane marker; the signed-bootstrap terminal fixtures changed
  an incarnation after anchoring the journal. Reviewed fixture-only corrections
  establish the final authenticated geometry before opening lifecycle authority,
  preserving all 17 file-level test names and 228 assertion sites.
- The eighth common build passes, and the same five cases pass without failures
  or ignored tests from `02:36:45.825396Z` to `02:37:33.901919Z`. All 60 selected
  files, including 54 Rust sources, and the copied binary remain unchanged.
  Core binary SHA-256 is
  `772c303807523c14507db5ce9e74ddb36a71e664df156ae5b0568b0b15ad543c`.
  V8 manifest SHA-256 is
  `5fe3b6f4f3a7c710df50e913cf6989f4e1f8943d1bddb546ccb1ce468eaad6b9`;
  the five-test selection V2 is
  `3f980336d36c7aca4191d7230ad0766d4b58a42b4d778179fda50ca47b0b8870`.
  Retained receipts and results are under
  `core-total-delta-retry8-provenance-review/` and
  `core-kura-total-delta-direct-5-retry8/`. The failed seventh-run evidence remains.
- Physical resource accounting is still private and incomplete. It must bind
  exact path ownership, directory identity across publication, nested recovery
  and verified deletion before a complete metric may be registered. Resident
  source review passes with 23 tests prepared; those tests are not yet compiled
  or executed. No resource or release gate closes from the five writer passes.

### 2026-09-08 source-fidelity diagnostic and resource qualification

- The complete `_exact_output_production_source_fidelity_errors` diagnostic
  finishes at `03:42:25.181097Z`, exit zero after 1,045.697 seconds, with zero
  errors. All 19,328 source type/mode/presence records remain unchanged at
  `716b0547b711dbe841a59da0460304b9c1f2e676ccd2acf2e90403d068a53d89`.
  An external Git view preserves the original HEAD and stage membership;
  actual include-closure positive and missing-provider negative checks pass
  before the run. This is a sealed-source diagnostic, excluding the later
  predecessor-anchor patch, not full proof-ledger or live-workspace qualification.
  Receipts are under `formal/source-fidelity-detached-20260908-v2/`.
- The first detached attempt lacked the Git context required by the real include
  resolver. Its 718 cascading diagnostics are retained as an invalid-environment
  failure, not classified as production-source defects. No checker predicate was
  bypassed to obtain the corrected result.
- The published predecessor-anchor correction separately passes all 36 selected
  controls with 69 focused source/config inputs unchanged. All 82 original AST
  assertions remain intact. Results are under
  `formal/source-fidelity-current-20260908/prior-predecessor-anchor-published/`.
- The 95-file composed Kura inventory and 13-file telemetry successor are
  published with reviewed compilation and fixture corrections. Physical scope
  remains nine Kura-owned trees, eleven exact root files and bounded validated
  process quarantine; delegated consensus files and the process lock are excluded.
  Fifteen physical and eight resident families must all be available before a
  complete metric can qualify. Publication receipts remain under
  `scaling-kura-composed-review/source-ready-v1/` and
  `scaling-kura-telemetry-export-review/`.
- Retry 19's exact 208-leaf selection finishes **206 passed, two failed**, zero
  ignored, with all 8,812 source paths and both original/retained Core/Config
  images unchanged during execution. Both direct reservation guards pass. The two
  remaining failures reach signed-snapshot bootstrap context validation after
  the earlier ordering fix. See `core-kura-direct-208-retry19-root/result.json`.
  Retry 18's separate 197-pass/11-fail result and all intermediate repairs remain
  retained; neither combined build is promoted to a pass.
- After the exact signed-bootstrap fixture correction, retry 20's retained Core
  image passes both affected snapshot finalizer cases and the related signature
  rejection test at `2026-09-08T09:53:44.772817Z`. Its 8,820 source records and
  binary identities are unchanged during execution. The same four-test receipt
  includes one separate BFV fixture failure; it is not a four-test pass. Evidence
  is in the `core-policy-tests/result.json` receipt under
  `target/privacy-release-evidence/2026-09-07-recovery/core-retry-20/`. The combined retry-20 build remains exit 101.
  The earlier 206 passing leaves and these three are separate source scopes.
- The private formal successor passes **65 cases and 17 subtests** against the
  exact historical 348-owner retry-19 source, with 1,840 captured files and 31
  absences. All 110 validator calls retain copied/rehashed inputs and a stable
  private index. The reviewed nine-file patch is unchanged from the previous
  candidate; 39 live Git providers remain explicitly unqualified. See
  `scaling-kura-composed-review/formal-contract-successor-v3-retry19-exact/`.
  This is not a complete model or live-index pass; older failed captures remain.
- The corrected private resource package passes **1,324 cases**, zero failures,
  errors or skips, with all 20 source inputs exact. It includes 364 parser,
  54 process, 181 HTTP, 108 worker, 190 replay, 259 budget/projection,
  85 typed-scanner, 76 ten-run experiment and seven package controls. The HTTP
  scope fix and experiment owner have independent source reviews. Actual worker
  publication and raw replay execute across all ten synthetic runs; this does
  not prove canonical lane routing or effects. The process suite's own Python
  RSS/image smoke and test-owned loopback/pipe work remain distinct from validator
  measurement. See `scaling-resource-policy-package-review-v2/validation-v1/`.
  The earlier 1,218-case package and component results retain their exact sources
  and receipts. Independent composition review passes; publication is pending and G-SCALE stays open.
- The coordinated retry-22 build passes with all **8,821 source paths unchanged**.
  Its exact 61-leaf CLI selection finishes **61 passed, zero failed or ignored**,
  with the full source inventory and original/retained CLI binary unchanged.
  It contains every original 57 leaf plus four new workload regressions; all 31
  resource Clock/IPC/parent controls pass. The published unique Clap group ID,
  actual metadata-value size and bounded frame-derived decoder allocation repairs
  resolve the prior failures. See `cli-load-direct-61-retry22/result.json`.
  Retry 21's separate 41-pass/16-fail receipt and independent review remain
  unchanged at `cli-load-direct-57-retry21/`; no failed cohort is relabelled.
  Exact routing/effect authority, launch identity, all writer allocations,
  main evidence-validator integration and the CLI pre-admission handshake remain
  open, as do all five paired measurements. M1–M6 and every release gate stay open.
