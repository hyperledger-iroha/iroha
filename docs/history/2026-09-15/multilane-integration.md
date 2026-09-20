# Multilane integration checkpoint — 2026-09-15

All six implementation milestones and all seven release gates remain open.
The implementation goal is active. The current changes are being composed and
validated in a private source snapshot before application to the shared code.

The snapshot starts from commit
`3fe824bcad43d5d961b8096da555d1bbff02310a` and includes the multilane integration
overlay. Its Cargo.lock is unchanged. This is an uncommitted local candidate;
the base commit identity is not a signed release identity for the overlay.
Runtime bounds continue to come from configuration. No compatibility decoder,
alternate consensus path, new crate, dependency or ABI version is introduced.

## Completed scoped validation

- The shared finality selection passes seven Rust tests with no failures or
  ignored cases. The canonical JSON failure reason uses one case-sensitive
  scalar representation and checked bounded serialization. The first compile
  failure is retained separately from the passing corrected run.
- The Rust SDK finality/readiness selection now passes all 41 tests, with no
  failures or ignored cases and unchanged source throughout the run. Its first
  run passed 40 and failed one stale expectation: a large proof returned with
  an error status correctly exceeds the 2,048-byte failure-envelope limit.
  The corrected test asserts that limit; production admission is unchanged.
- Torii now passes all 39 selected tests with unchanged source, no failures
  or ignored cases, and every requested filter present. All four actual-handler
  cases ran: signed success/tip race, startup/conflict classification, admission
  precedence and missing/corrupt durable proof. The typed HTTP 500 failure and
  proxy body/authority regressions also pass.
- Formal include reconciliation passes 22 focused tests across nine source
  owners. Twenty explicit include edges are added while preserving prior
  edges/order and all semantic checks; missing, duplicate and extra providers
  are rejected. This is not a full structural or formal-engine result.
- Semantic binding reconciliation passes 76 focused tests, including actual
  owner resolution, missing-provider rejection and behavioral-drift controls.
  The complete structural check and formal engines remain unqualified.
- The current Python inventory collects all 4,761 prior node IDs across 66
  suites and binds 264 sources. Collection is not test execution.
- Canonical preflight tests pass 101 cases, archive tests pass 96, and the
  bootstrap validator passes all 50. The original framework-copy helper also
  completes 130 real subprocesses; the validator reaps 138 Python children.
  These runs preserve their complete 276-file execution snapshot.
- The isolated worker selection passes 190 cases, including actual child
  execution using a standard-library-only fixture. The child no longer imports
  pytest through its fixture construction path.
- Fifteen real Bash 5 cases check descriptor handoff, external-helper
  isolation, permanent close and failure handling. Trusted Bash forks retain
  the runner's authority; the contract explicitly excludes them from the
  external-helper isolation claim. Unsupported Bash 3 fails before execution.
- The legacy-codec guard passes in the independent Git/source context.

## Current implementation and remaining execution

Torii integration compiles after two corrections. The readiness enum documents
its intentional size difference: its large attestation is boxed, while a small
failure reason stays allocation-free. The lifecycle runner's post-finalization
drain passes the existing control-queue capacity, matching the ordinary call.

The first Torii runtime selection passed 23 of 26 tests. Generic HTTP 500
sanitization incorrectly erased a valid typed finality failure; the candidate
now reconstructs its fixed public envelope after validating its sole detail and
exact status, while preserving generic error sanitization. Two proxy fixtures
also needed correction for early Content-Length rejection and the actual
committed authority height. A broader run passed 34 of 35 executed tests,
exposing an incorrect fixture length and four unregistered handler cases
accidentally nested under optional WebSocket tests. Moving those cases into
the default test module exposed a nonexistent route constant, corrected to
the production route catalog. The final 39-test run passes; all original
failures are retained. The private native driver now rejects absent requested
filters, empty execution and ignored tests, with five audit controls.

The original formal structural check failed with 61 distinct diagnostics.
After reviewed include and semantic corrections, the complete source295 check
reports 77 diagnostics; newly resolved owners expose further binding checks,
so the counts do not measure a runtime regression. The combined focused run
passes 112 of 113 tests, with no skips. Its remaining failure is the corridor's
Kura application-receipt production-source fidelity check. A separate State/lane
repair passes all 145 semantic controls, preserving the earlier 76; composition
and a complete rerun remain pending. Native Kura and in-flight accounting binding
repairs are in progress. These checks do not execute TLC or Apalache.

The P2P include inventory now includes the actual connection-lifecycle tests;
seven real resolver controls pass. Actual collection of the updated mandatory
proof-fidelity selection finds 6,007 unique tests, and its registration is
composed. Collection supplies no full execution result. Further test additions
require fresh canonical collection after composition.

The bounded TODO audit removes two temporary test-only released-Apply traces
while preserving both ownership-returning failure branches. It replaces a
stale physical-accounting TODO with the implemented ownership contract and
records emergency Fast startup's non-productive posture. Five released-Apply
regressions and fifteen lifecycle drain controls join the Core selection.
Core compiled and selected 102 tests, then aborted after 41 emitted passes with
a stack overflow in `merge_entrypoints_commit_in_canonical_carrier_membership`.
The crash report locates the overflow during autonomous fixture setup, before
its membership assertions. That test used the default test-thread stack; the
candidate now uses the existing production consensus thread builder, matching
the related autonomous tests and retaining the configured 64 MiB stack budget.
The complete corrected selection now passes all 102 tests, with zero failures
or ignored cases and all 34 requested filters represented. Source hashes remain
unchanged throughout the 626.9-second run. The earlier abort is retained as a
failed attempt; the passing rerun covers its entire original selection.

The current CLI readiness/transaction-load build fails before test execution
with seven compile errors. A six-file repair packet preserves old assertions
and adds nested secret-erasure and typed-error checks; compilation and runtime
validation of that packet remain pending. Kagami's current scaling/localnet
build also fails before tests, with 47 compiler errors, mainly obsolete Norito
JSON macro paths plus API, type and borrow mismatches. Its repair is in progress.
Both build receipts record unchanged source; neither is a test pass.

The autoscale fixture now selects the default production daemon for ordinary
recovery and lifecycle scenarios. Only the scenario that injects authenticated
Hold/Drop messages selects the separate message-control daemon. The sidecar
restart and two-phase drain tests assert that their peers have no message
controller. Formatting and source review pass; network execution is pending.

The original complete preflight owner is running all six preliminary phases and
66 registered suites against its frozen pre-lint snapshot. Its real canonical
source digest is
`3a0f24ccc2c5789e6a1dacd19f66fc04cf52d694dc19c0cb28f387f09f3a7d2f`.
A terminal result is required before claiming that complete run passes.
At the latest checkpoint, 31 of 72 units had published passing reports covering
2,581 of 4,898 required outcomes, with no reported failures or skips. The later
native corrections require a fresh canonical inventory and final-source checks.

Remaining qualification includes the complete native and SDK selections,
current source-bound formal positive and mutation results, four-validator
networks, ten corridor seeds and the two-hour soak, five pinned one/four-lane
measurement pairs, and workspace build/test/lint checks. No scoped result here
establishes release readiness.

## Retained local evidence

The ignored `dist/multilane-validation-20260907/` directory contains:

- `multilane-shared-integration-20260915/`: composition, source inventories,
  retained compiler failures, the passing `shared-finality-json-fix` run, and
  subsequent native invocations.
- `scaling-combined-actual-validation-20260915/result.json`: sealed actual
  archive/preflight/framework/bootstrap-validator execution.
- `scaling-post-review-inventory-collector-20260915/`: the actual collection
  receipt and proposed registry.
- `scaling-actual-python-process-slice-20260915/`: worker failure and corrected
  execution evidence.
- `scaling-bash5-fd-qualification-20260915/`: actual descriptor observations and
  the documented trust boundary.
- `scaling-autoscale-production-fixture-split-20260915/` and
  `scaling-four-validator-launch-matrix-20260915/`: reviewed fixture changes and
  exact network launch prerequisites, without an execution claim.

## 2026-09-16 status checkpoint

The active goal and all six milestones/seven release gates remain open. The
candidate contains 302 files; the shared checkout still has only status/history
edits from this integration task. Applying the final reviewed implementation
and validating one complete source candidate remain required.

- Focused native passes remain seven shared-finality, 41 Rust SDK, 39 Torii and
  102 Core tests, each tied to its recorded source snapshot.
- CLI and Kagami now compile. The complete CLI selection passes 205 of 207 tests
  with no ignored cases. Two aggregate-cap fixtures were corrected against the
  existing four-term production budget and reviewed; the full rerun is pending.
  Kagami's original full selection is still running and has runtime failures in
  localnet/signing fixtures. Descriptor-based seeds and canonical private paths
  have reviewed fixture repairs; default NPoS staking-asset alignment and further
  signing corrections are under review. No Kagami runtime pass is claimed.
- State/lane 145, Kura in-flight 46 and Native 42 plus seven original negative
  controls pass in their isolated scopes. Their binding corrections are composed.
  The previous combined check remains 112/113 with 77 structural diagnostics;
  its corridor fixture is being repaired. The source302 structural attempt failed
  because its private Git index was uninitialized. This setup failure requires
  an index repair and fresh full run, and establishes no formal-model result.
- The scan-boundary correction passes 109 controls. This is tooling evidence,
  with no throughput or latency measurement.
- The original 72-unit preflight terminated naturally after 47 units. The first
  46 units passed; `scaling_public_files_test.py` then failed two of 63 tests.
  Overall, 3,617 of 3,619 executed outcomes passed, with zero errors or skips.
  All owned children terminated and the source census is unchanged. The remaining
  25 units did not execute. The original failure is retained and under diagnosis.

Remaining work is to finish these concrete failures, compose and apply the
reviewed implementation, refresh canonical test inventories and qualify formal
positive/mutation models, real four-validator recovery/lifecycle networks,
10 corridor seeds using 13 global validators and three four-validator dataspaces,
a two-hour fault soak, five pinned one/four-lane performance pairs, maintained
SDK parity and full workspace build/test/lint checks. No compatibility path is
introduced; the first-release design remains canonical-only.

## 2026-09-16 shared application and composed checks

The reviewed candidate was applied to the shared checkout: 287 changed files,
including 183 new files, from the exact recorded HEAD preimages. Every applied
hash/mode matched; the Git index and existing status/history edits were preserved.
Seven whitespace-only corrections preserve identical Python ASTs. Later reviewed
localnet/signing, public-file and generated-help corrections are also applied;
the current candidate inventory is 304 files. Cargo.lock remains unchanged.

The complete source302 structural checker now passes with zero diagnostics in
103.8 seconds after initializing its private Git index. All 276 combined include,
State/lane, Native and in-flight positive/mutation regressions pass in 282.6 seconds,
with no missing selectors, failures or skips. The complete source remains unchanged.
These results establish the checked bindings and regressions, not a production
trace-refinement theorem or a complete formal release gate. The five current TLC
positive scopes and the original 106-case mutation runner are now running privately.
Two older 18-step Apalache processes remain confirmed live and unqualified; neither
was signalled or restarted.

Kagami's complete original run terminated naturally: 478 selected, 406 passed,
72 failed, zero ignored, and all seven requested filters represented. Its source
manifest remains `dd400f5b9fb573b507f582b160d95fe2063826b9cfc734fecc94c5bac16aeb75`.
The retained log SHA-256 is
`b7d80cc1ebcb747206d280fa80e5bdf4f4b5749831521c2a711a28300dbb527d`.
Runtime/help corrections address its shared seed/temporary-path fixtures, complete
signing inputs, canonical default NPoS stake selection, actual generated operator
identity and obsolete generated help. All original test selections remain required.
The help snapshot is taken from the actual clap-markdown generator output; manual
scaling input contracts now live in the source-coupled scaling-gate specification.

Both original public-file failures were reproduced with 600 held descriptors.
The tests searched only descriptor numbers below 512; the full preflight retains
both dependency package trees while running them. Capturing the actual owned stage
open fixes the fixture without changing production. All original 63 cases pass in
both normal and pressured execution, retaining every original node identity. That
single test owner is applied; the full preflight rerun remains outstanding.

After the original Kagami run terminated, the complete native build source was
advanced to manifest
`9af076207b95d252f97c71c61298b56ff76c709c7c76107f9ead15d4356d25c2`
(20,650 entries), with a fresh process inspection before synchronization and Cargo.
The original complete CLI selection is running against that source with both
aggregate-cap fixture corrections. The applied source passes whitespace and retired
codec/compatibility guards. No milestone or release gate is closed.

The corrected full CLI run now passes 207/207 with zero ignored cases in
158.7 seconds, retaining the unchanged `9af07620…` source. The bounded streaming
crypto selection also passes all five tests. The default production daemon build
is running against that same frozen source. Current TLC evidence is sealed at
`scaling-current302-tlc-20260916/result.json`: all five fixed models pass and
all 106 original mutants produce their exact expected counterexamples. The
configured bounds and recorded inputs are unchanged. This does not qualify the
unexecuted in-flight/Apalache, deductive or production trace obligations.

### September 16 current status checkpoint

The default production daemon build completed successfully in 882.013404708 seconds on unchanged source `9af076207b95d252f97c71c61298b56ff76c709c7c76107f9ead15d4356d25c2`; its original session was 53229. The four-validator integration harnesses and runtime suites remain pending.

The separate reviewed corridor fixture selection passed all 60 collected tests in 689.98 seconds with its complete source census unchanged; composition and canonical recollection remain pending. The four original JVM parity harnesses passed 127 tests without failures or skips: grouped Kotlin 11, grouped Java consumers 7, diagnostics Kotlin 50 and diagnostics Java consumers 59. Their complete 3,230-file frozen SDK census was unchanged. The SDK source-closure suite remains 21/23 because current finality OpenAPI artifacts disagree; current Rust fixture/OpenAPI regeneration and native Python, JavaScript and Swift artifact/parity execution remain pending. These results do not close any milestone or release gate.

### September 16 build and network checkpoint

The reviewed corridor, SDK source closure, Apalache scheduling, authored OpenAPI
and in-flight recorder changes are now composed in the shared 320-path candidate.
The recorded Git index and Cargo.lock remain unchanged. Different validation
captures retain their own source identities; their passing subsets do not qualify
the entire changed candidate.

Both original four-validator integration harnesses compiled successfully in
743.297773458 seconds on unchanged source `9af07620…`. The first full ten-iteration
rotating-validator Native attempt started all four peers, then failed on its first
submission because a blocking SDK method was called inside a Tokio runtime. The
test reported zero passes, one failure and no ignores; the original process exited
101 after 54.799473708 seconds. Source and both executable hashes were unchanged.
All four owned peers shut down normally and their artifacts remain retained.
Canonical async submission, query and diagnostics integration is still required
before rerunning this scenario; no four-peer release pass is claimed.

The corrected complete Kagami invocation also terminated on unchanged `9af07620…`
source, after 229.000779959 seconds. Six genesis-signing cases rejected a fixture
directory without owner-only mode 0700. The later retained scaling-facts fixture
overflowed its stack and aborted the process before the remaining filters ran.
There is no complete 478-case summary for this invocation. Its log SHA-256 is
`9701abdcc3f7f577bbdd97dfd652af5909a5da1c752146534549ef4bf6c3e32e`.
Both fixture failures require correction and a complete rerun with all original
assertions; production custody checks remain enforced.

All five other Apalache positive models pass at their original bounds 8/8/12/8/8,
with every named invariant checked, exact inputs unchanged and empty stderr.
The reviewed schedule checks invariants after joining transitions; no model,
invariant or bound is removed. Evidence is retained in
`scaling-apalache-five-after-join-20260916/result.json`. The separate 18-step
in-flight run remains live and unqualified, as do its untouched older attempts.
Deductive verification, production correspondence and complete formal release
qualification remain separate obligations.

The corrected SDK source-closure selection now passes all 23 cases on an unchanged
23,401-entry capture. The authored OpenAPI changes match the current finality
implementation; generated release metadata, signatures, canonical Rust fixtures
and current native Python, JavaScript and Swift qualification remain pending.
The separate JVM result remains 127/127. These are scoped SDK checks, not G-SDK.

Actual collection now finds 6,170 unique proof/source nodes across 23 selectors
and 4,781 scaling nodes across 67 suites. Collection executed no test bodies;
final registration, source closure and complete runner execution remain required.
All six milestones and seven release gates remain open. Remaining network evidence
includes ten corridor seeds with 13 global validators and three four-validator
dataspaces, a two-hour fault soak, and five pinned one-lane/four-lane performance
pairs meeting the unchanged throughput, latency and resource thresholds.

### September 16 complete in-flight TLC and native fixture continuation

The original in-flight runner now completes all 26 required cases: one positive
model and all 25 exact named mutation counterexamples. The original runner exited
zero in 103.641784208 seconds, every case retained empty stderr, and the complete
20,649-entry private source remained unchanged. Its source manifest is
`22ba6c2b73cc8cf0cd6e02a36de272fa2e6c4c211485a452916b6f7f0029a7fe`;
the per-case summary is
`4f02032d7a83256f89834185bd930b3516fe185f9a27cff1b5ff3a57cc852aa2`.
The source/recorder correction separately passes all 33 selected regressions.
These finite model results leave the 18-step Apalache, deductive, production
correspondence and full formal release obligations open.

The eight-owner registration for the actually collected 6,170 proof nodes is now
applied. It preserves all 21 original selectors in order and adds the two reviewed
Native/in-flight recovery suites. Complete execution is still required.

The Kagami directory and scoped facts-fixture stack corrections are applied.
All 40 original facts test names remain; 30 complete generated-genesis fixture
bodies execute on a joined, bounded worker with the caller's account discriminant
and original panic propagation. This preserves the production command path and
does not establish ordinary CLI main-stack sufficiency.

The full original 478-test invocation is running on unchanged native source
`5209c6ed96667cb51bc659959608fadb0908079ef827effb116c0fdd941f09ed`.
It passed the earlier stack-abort point and exposed a default-account network
prefix mismatch, a no-clobber fixture overwrite and an exact signed-request route
mismatch. The first two have reviewed test-fixture corrections in the mutable
candidate: render the omitted disabled VPN default account for the manifest's
network, and write the malformed identity to a fresh private configuration.
The route mismatch remains under diagnosis. The active native source and original
process are preserved; no complete run or release success is claimed.

### September 16 asynchronous SDK and staged routing integration

The shared candidate now contains 339 paths. The reviewed async repair changes
22 owners and documents the canonical SDK calls in the crate README. Account
queries share the existing typed request construction and strict response decoder;
each continuation consumes its cursor before I/O and cannot restart after failure
or cancellation. Diagnostics uses operator-signed async transport with its complete
evidence validation. Synchronous consumers use the explicit blocking facade.
Native submissions preserve the exact signed transactions, fee-quote bounds,
event subscription order, fault acknowledgements and exact-once assertions.
Review corrected a shadowed frontier vector before shared application.

The private proposal passes 11 formal controls and formatting for 19 Rust owners.
The applied successor includes two reviewed source corrections; those checks do
not establish its native execution. Thirteen focused Rust cases and fresh network
qualification remain pending. The retired-codec guard passes after application.
The composition receipt is `async-sdk-composition/applied-result.json` within the
shared integration packet. Canonical proof and SDK inventory regeneration is in
progress and does not reuse the earlier source-closure pass for the new files.

The Kagami failure is a production join defect: canonical account metadata writes
require state, but facts assembly requested a stateless route. The applied repair
resolves every original request during the authenticated genesis StateBlock borrow
for each peer configuration. Each retained authority owns the ordered signed-byte
hash and exact single route. The caller charges the shared decode and all four
compact projections before staging; the complete stopped-tip finality and effect
verification is unchanged. No fallback route or alternative workload is added.

The five-owner repair passes source review, exact forward/inverse patch replay
and formatting. All 13 assembler cases retain their semantics, with one corrected
test name and three new cases for allocation bounds and failed staged projection.
Native execution remains pending. The receipt is
`facts-state-routing-composition/applied-result.json`; its supplier result hash is
`2226aceb95be376574508cc4ddde5b3b97d8a4671c5ae89bf67d76e27abaad9a`.

The complete 73-unit preflight is running as original session 70296/PID 86626
against a frozen 320-path candidate. Its first 11 units pass 1,096 outcomes.
It requires all 4,918 outcomes and 294 archive files; no terminal result exists
at this checkpoint. Its actual workspace source manifest is
`8027889ccc207cc0032c0ec7a7aca88801aa20e64dc0fe3ae9bba763d58b935d`.
The later SDK and routing changes are excluded from that run. The original Kagami
478 selection remains on its unchanged `5209c6ed` source, and the 18-step Apalache
attempt is still live. All milestones and release gates remain open.

Actual async proof collection then completes with 6,172 unique nodes across
24 selectors, preserving the original 6,170 nodes and 23 selectors in order.
The seven-owner registration and all five component-hash references are applied;
the complete execution wrapper changes only its four corresponding count bounds.
The canonical cross-language SDK resolver still reports 914 grouped and 918
diagnostic records: its declared closure roots exclude the new Rust query files,
so its manifest is unchanged. The separate release-selection audit found all
13 new Rust regressions unregistered; their exact execution obligations are being
added before the final candidate capture. Collection and source resolution do
not count as executing those tests.

### September 16 exact async Rust release registration

The reviewed 19-owner composition registers all thirteen async Rust regressions
across five counted release commands: nine query cases and one each for async
diagnostics, the explicit blocking boundary, shared request encoding and Native
typed results. The corridor now requires 88 legs; the original 531-test G-UNIT
selection is unchanged. Exact named results are required independently of the
aggregate counts. Missing, substituted, duplicate, extra and Unicode-extra
results are rejected. Thirty extracted-parser controls pass; complete current
receipt and native execution remain pending.

Root review found missing new-leg name mappings in the validator and successful
receipt fixture. Those are repaired without removing the earlier diagnostics
assertions. An actual earlier 27-control run passed 26 and failed the fixture's
stale bootstrap fingerprint, with its complete source unchanged. The exact
baseline-to-current bootstrap change is retained and its production and fixture
fingerprints now bind the composed owner. Both canonical grouped-fixture source
bindings follow the reviewed validator component, preserving their semantic
checks. The six count references and one existing count-drift negative move
from 83/82 to 88/87 together. Final collection must verify that sole parameterized
node-name change; no compatibility alias is introduced.

All 342 candidate file hashes and modes match shared source and both mutable
preparations. The shared index and Cargo.lock are unchanged. The composition
receipt is `rust13-registration-review/applied-result.json` in the integration
packet; its candidate-pins hash is
`e2ad757a962b295a4cab19ba9660ef966422bdf4eeab92bdb91bf44be2a8bc17`.
Python 3.12 parsing and the pinned Bash 5 syntax check pass. The initial check
used a missing Homebrew Bash path, and the system Bash 3 cannot parse existing
descriptor syntax; neither attempt is treated as source validation. The final
scaling inventory and proof collection are being regenerated before freezing
the complete execution source. Original live native, preflight and formal
processes remain on their unchanged sources. Every milestone and release gate
remains open.

The actual final 342-candidate proof collection completes with 6,172 unique
nodes across 24 selectors. All 6,171 other names are unchanged; ordinal 3,066
is the same count-drift negative with its implicit parameter name updated to
88/87. Its complete source remains unchanged. The 37-control execution then
records 33 passes and four failures: two private-index-readiness failures and
two receipt fixtures still invoking the retired `RunReplayInput` validator.
Those failures are retained, not counted as passes. The two source controls
are rerun only after verified index completion; the receipt fixture is being
migrated to the current parent execution record, without a compatibility API.
The full 73-unit preflight separately reaches 26 completed units and 2,321
passing outcomes on its original frozen source; it is still running.

The canonical scaling inventory collection then passes on the frozen 342-path
composition: all 4,781 identities across 67 selectors are preserved. The applied
inventory updates thirteen reviewed source fingerprints while retaining all
268 source names, 137 phase cases and 101 explicit migration mappings. Its
SHA-256 is `5b2c6abf6faeed49397da6c541a5ad566e25531e5adc8092f4c5c17136e5d5e9`.
The unchanged root collector retains an ambiguous descriptor-relative LICENSE
audit label; no exact source referent is asserted from that event. The complete
source census and separate absolute-read diagnostic remain available. Collection
is not execution evidence.

Three reviewed source-coupled documents now describe the fixed descriptor-based
collector and canonical parent execution record, replacing the obsolete shell,
standalone validator and externally supplied evidence instructions. Their
localnet/facts section remains byte-identical. The current candidate-pins hash
is `552e2a938e91466888ebfb370232da9a4dd0e018e302a42d0c5f4dc26320d6ef`.
A complete structural check is running as original session 41859/PID 92729
on that prepared source; it must remain unchanged through terminal census.
Receipt fixtures and normalized approval declarations still need their current
interface migration. The shared whitespace check and historical archive
verification pass (64,736 records and 67,311 occurrences). No gate closes.

After the private index completed, the original two-source-control retry
10666 passes both cases with no failures/errors/skips in 665.335 seconds. All
23,405 source entries, dependency bytes and admitted index remain unchanged.
This resolves the setup failures for that frozen composition; the original
33-pass/four-failure result remains retained. The two current receipt failures
still require fixture migration.

The complete current342 structural check now finishes naturally with exit 0
as original41859/PID92729 in 107.540 seconds. All 23,405 source entries and the
private index remain unchanged; stderr is empty. This binds the current model
and production-source contracts without claiming proof-engine, runtime or
release qualification. Its result is `current342-structural/result.json` in the
integration packet. The independent full73 run reaches 32 completed units and
2,601 passing outcomes on its own frozen source; it remains live.

The approval contract now requires eight network operations, including complete
scaling preflight, five paired trials, and retained verification/publication.
Three focused tests pass, including seventeen rejection variants for the retired
plan and omitted obligations. Four source owners and the one registered liveness
pin are applied; the candidate grows to 345 paths. This does not execute or
qualify any network or performance experiment.

The receipt bootstrap archive fixture now uses the canonical execution-record
path and digest. The complete original bounded atomicity helper passes in 9.509
seconds with all forty original assertions and two added substitution controls.
Its old source reproduces the incomplete-validator-invocation rejection.
Independent review and exact source-pin composition are retained in
`current-receipt-archive-fixture-composition/applied-result.json`. The current
345-path candidate-pins digest is
`6a27776525319ab71c203fcbca5dae099d8baf7e6a500bab2801e41d60b16304`;
the scaling inventory digest is
`627eaca3eaf494bd7d66c8b3ff7b8f97431bb00ab579efc2e16dba66456d6b96`.
The containing broad pytest case and full receipt suite remain unexecuted.

Main receipt fixture migration exposes two production defects: `build_receipt`
reads four removed scaling input locals before verification, and generic
corridor/retained-directory validation still refers to a removed scaling path
regular expression. Their current-interface repairs are being developed with
the complete component/bootstrap fingerprint chain; no compatibility interface
is restored. Standalone bootstrap fixtures are also being migrated. Original
Cargo 84991, preflight 86626 and Apalache 65458 were freshly observed live;
their frozen inputs are unchanged by these compositions. All milestones and
release gates remain open.

The independent protected-module-load audit resolves 3,932 global-name loads
across 270 code objects. Besides the three artifact-path uses, it confirms a
removed scaling-named constant still referenced by the formal Apalache evidence
reader; the intended bound remains 16 MiB. No further unresolved globals are
found when generated methods are checked against their actual namespaces. This
is name-resolution evidence only. All three production repairs remain private
pending their current positive/negative tests and coherent source composition.

The stale CI requirement for the removed standalone scaling validator is now
replaced by the successful sealed-child/G-12P completion, original parent
handoff, failure propagation, channel closure and receipt-publication ordering.
The complete source suite passes fifty cases: all thirty-one original cases and
nineteen additions. Independent review caught a missing failure-assignment
binding; the corrected contract binds the complete two-root identity comparison
and rejects failure suppression, inverted comparison and omission of the sealed
root. An existing fragment fingerprint is refreshed only after exact retained
preimage comparison proves the approved sixty-five-line additive Rust test
registration, with no removed lines. The original failed test and first review
counterexample remain retained.

Canonical isolated collection on that 345-path composition passes 4,800 exact
identities across all sixty-seven suites in 23.509 seconds, with the complete
source unchanged. The canonical outcome-ID producer confirms all 4,781 old
identities remain plus nineteen CI cases, with sixty-six suites unchanged. The
newly read CI source also needs explicit admission in the preflight source
allowlist and inventory; collection alone does not close that dependency.
Current candidate pins are
`ec15de96fc9af2b9f4556ea965ca21c8e4017ee26a8a2ecb06ed4fbd8dc4a2eb`.
Main receipt, standalone bootstrap and retired-entrypoint source/test migrations
continue independently. No full CI, native, scaling or release gate is closed.

The private CI source-admission successor now passes all 198 original-driver
checks (96 archive and 102 inventory), with the complete source unchanged.
Canonical collection matches 4,801 identities across 67 suites and 269 sources;
all prior 4,781 identities, 137 phase cases and 101 migration records remain.
Root independently reviewed its exact allowlist addition and inventory changes.
The successor remains pending composition with the reviewed Kagami journal
fixture correction; neither collection nor these two suites qualifies the
complete preflight. Its result is
`scaling-ci-source-admission-20260916/result.json` under the ignored evidence root,
SHA256 `194643dae8bbf9793b06188805cc40e219622e27e9db15a604a85a496cc14749`.

Two further receipt/bootstrap defects are confirmed in addition to the three
name-resolution repairs. The final bootstrap completion marker changes the
same directory frozen as the scaling execution record's parent. The fix under
development separates immutable execution evidence from terminal output while
retaining the directory guard. Separately, authenticated scaling plan and budget
file contracts are lost before the final receipt publication guard. Their exact
first-read identities must survive through publication; recapturing a later
baseline would miss intervening replacement. Both repairs and their adversarial
controls remain private. The latest 23-case receipt run retains its actual
16-pass/7-fail result. M1–M6 and all seven release gates remain open.

The CI source-admission correction and coherent Kagami global/local journal-height
fixture are now composed into the working tree and both mutable preparation
trees. All 345 candidate hashes/modes match across those trees and the sparse
candidate overlay. All 269 registered source hashes match the three complete
trees. The inventory retains all 4,801 collected Python identities; its sole
additional change from the tested CI successor is the reviewed Kagami fixture
digest. Current pins are
`52f99fd1979fdae1d3305fb63e9d2b4cc0b3640d83ddd3ad24e8332130b00ba2`
and the inventory is
`6ab47bf91bb3166bec0a9bdf57d78244843ab71f000be4d7225c2b5d1562d0b5`.
The original composition command's postcheck mistakenly expected every full-tree
dependency in the sparse overlay and exited after applying the four files. That
diagnostic remains retained; the independent complete-tree and overlay checks
above confirm the applied state. The index and lockfile are unchanged. Kagami
execution remains pending, and the running native source was not modified.

The original complete-preflight attempt ended naturally after 7,438.414 seconds.
Its first 61 units pass 4,366 outcomes; unit 61 then passes 45 and fails eight
of its 53 handoff cases, leaving eleven later units unlaunched. All 23,401 source
entries remain identical and the original dependency admission/census audit
passes after owner release. The exact terminal result is
`scaling-all73-original-owner-20260916/run/result.json`, SHA256
`b47bd66f1b0d2c563e2d7c792483001d619a03dab687f5c82d1c2da5a33f2ebb`.
A separate original-source reproduction preserves the same eight failures:
the handoff parent uses `select()` on admitted descriptors above its fixed
descriptor-set limit. The initial failed unit's stdout was not retained by its
owner; the reproduction's complete diagnostics are retained separately.

The independently reviewed readiness correction uses the existing selector
abstraction, closes each readiness owner, and rechecks the original socket
identity before reading or replying. All 53 original handoff outcomes and seven
new high-descriptor, cleanup and foreign-reuse controls pass on 23,406 unchanged
source entries. The first successor's two overly strict new wait-count assertions
and their failed result remain retained; the corrected test preserves exactly
one successful wait and the existing negative-path natural cleanup rule. The
final two-owner result is `scaling-handoff-high-fd-successor-20260916/result.json`,
SHA256 `b2612e09cd16f299a5afe664af156bb1ae58c213d5bb35fcf48b70e12dc07982`.
The correction is private pending coherent receipt/bootstrap composition.

The retirement review now maps all 95 original assertion sets to 79 current
test definitions. Three initial lane-identity mapping gaps are replaced by
explicit generator count disagreement, duplicate independent-plan lanes,
re-signed entry membership drift and coherent alternate four-peer catalogs.
The two existing native test functions retain their original assertions and
names; their expanded execution remains pending. The separate preservation
suite passes 156 cases plus 58 subtests. The ten obsolete files remain present
until current fixture consumers and canonical inventory closure are composed.
All 101 historical migration identities remain intact. No release gate closes.

## September 16 — completed Kagami selection and reviewed fixture corrections

The original `kagami-scaling-and-localnet-private-directory-and-facts-stack`
selection ended naturally with 450 passes, 28 failures and zero ignored tests
after 10,020.90 seconds. Its complete source census is unchanged at
`5209c6ed96667cb51bc659959608fadb0908079ef827effb116c0fdd941f09ed`.
This is a failed selection, not release qualification. The earlier state-aware
routing, genesis and coherent journal-height corrections address the previously
diagnosed failures; a corrected complete execution is still required.

Six newly surfaced localnet failures were traced to five direct writer tests
that omitted production private-directory preparation and one funding assertion
that omitted the existing alias-setup allocation preceding the faucet allocation.
The three-file fixture correction uses the actual empty-private-directory owner
and checks both allocations exactly once in order. The original custody,
network identity and no-overwrite assertions remain. Independent review
`f8123c9a27620a2e2c6c22a464a83fbde3b9f1969453c9e8e6f557c5adddc0d8`,
Rustfmt, and forward/inverse patch replay pass; native execution is pending.

These three files and the two independently reviewed lane-ID preservation test
owners are now applied. The candidate contains 346 paths with manifest digest
`230681af9b19b3a182617c1950ce0c61810dd98c8cabbec05c215702b1259702`.
Only three existing canonical scaling source digests changed; all 269 sources,
67 suites, 4,801 Python identities, 137 phase outcomes and 101 migration records
retain their existing scope. The new inventory digest is
`9bb4fb77cb68522c03e7d94dcda5a05725b0306e49821a8d2411f0a4e130a5bd`.
All candidate bytes/modes match the four mutable preparation trees, and all
inventory sources match the three complete trees. Git index and Cargo.lock are
unchanged. The first composition precheck found a newly changed native test file
absent from the sparse overlay; it stopped before mutation. The corrected
composition verifies its original bytes in all complete trees and adds that
explicit owner. No historical source/result was rewritten.

A fresh process census showed no Cargo/rustc owner before advancing the native
snapshot to `22b0e597bc3c8f32347eb0a7330001eeb6ca9acfd7077d5fdab1a926c1d22d26`.
The original eight-test localnet/genesis correction run is active in session
48903. Its first driver invocation with Python isolated mode failed before
launching Cargo because the local audit-module import was excluded; the
unchanged driver was then invoked with its ordinary module path. No test pass
is inferred. All six milestones and seven release gates remain open.

## September 16 — native fixture execution and receipt successor evidence

The first focused eight-test attempt failed compilation in five calls that
passed wire `ScheduledV1` records to the runtime `ScheduledRequest` verifier.
It ran no tests; source `22b0e597bc3c8f32347eb0a7330001eeb6ca9acfd7077d5fdab1a926c1d22d26`
remained unchanged over 405.89 seconds. The reviewed test correction reuses
production `RequestV1::into_parts` and retains every route-negative assertion
and the later actual preparation/replay. The corrected native selection
finished seven passes/one failure in 78.35 seconds, with source
`e64393a26ae4cd91ef68847da39483b2eeafa1b91ff0239a06155012334fc3cb` unchanged.
All six localnet corrections and the genesis hash-placeholder rejection pass.
The remaining Taira negative reached configuration parsing and found omitted
governance accounts encoded for the default network. Its fixture now fills
only omitted defaults through the existing explicit network rebinding helper;
all supplied values and the intended canonical-XOR rejection remain intact.
Independent review `4d8d9aadc108e9b84706d4077a90262e95f2efa622a10abf25adaf40bd1b8824`
and Rustfmt pass; that runtime rerun remains pending.

The separate original three-test native selection ended naturally after
214.95 seconds with two passes/one failure and unchanged `e643` source. Both
new lane-ID preservation tests passed with all original assertions. The actual
generated-facts positive failed because its eight-leaf fixture cap is below
the generated genesis count; the independent correction is in preparation.
The complete selection is failed and does not qualify the release.

The final shared candidate contains 346 paths, manifest
`67c3d3398ccabea12c775a416b9ee1d6fc9d9aafd8ace537eb3bc58e18f66242`;
its 269-source inventory is
`89f322886952442e55e21d2f6002d75ff0d515fe1ef4110ee1e0fa65cb6b81d0`.
All four candidate overlays and all three complete inventory trees match.
Index and Cargo.lock remain unchanged. After the previous native owner exited,
the next source snapshot became
`3573289b9b77328c4ea59a7e61c2df8c540d681661ec1cc027e1bf541aec0fd0`;
the original nine-test async-query selection is running in session 40479.

The private receipt successor passed all 24 selected controls over 688.64
seconds with 23,405 source entries and admitted dependencies unchanged. Its
result is `5c45fd7f72aef35dffd57c547bd4137745d5d933171abf641528cf62a272c8c2`;
independent final-chain review is
`67a8c878b1ae1c6305aadf563cfc2bd20ea07da902a939261a41f7c954221dfc`.
The private retirement producer collected 4,841 identities across 67 suites
and 270 sources; 156 preservation tests and the physical ten-file absence
control passed. Neither packet is yet the final shared composition. The
receipt gate now has an independently checked 375-identity successor, retaining
all 370 existing identities and order, adding four fixture controls and one
strict pytest-duration parser regression. Its final joined execution is pending.

The next local network driver also checks exact emitted binary identity both
before and after execution against completed matching-source build records.
Thirteen controls pass on Python 3.12.14, including equal-byte inode replacement,
mode change and missing images at the terminal fence. Independent review
`f4efe655b9baa0d9cc993eb483b0672680146361b7ecec9fd231ae4853361ba0`
passes. These helpers have not yet launched a build or network and establish
no release or runtime network qualification. All milestones/gates remain open.

## September 16 — async SDK query compilation blocker

The original nine-query native selection ended naturally with exit 101 after
200.93 seconds. It ran zero tests: three `Send` lifetime errors in the async
query collection/single-result wrappers and four type-inference errors in their
tests prevented compilation. The source manifest remained unchanged at
`3573289b9b77328c4ea59a7e61c2df8c540d681661ec1cc027e1bf541aec0fd0`;
the log digest is
`4bfed65f525954bdd917b5057367599b7a4a70702d072ba56545de837dcf5a8f`.
The corresponding result is retained at
`dist/multilane-validation-20260907/multilane-shared-integration-20260915/native-runs/sdk-async-query-nine/result.json`.
Repair is in progress. No SDK or network pass is inferred.

The generated-genesis fixture leaf-cap correction is independently reviewed
(`123036165bc26cf33e8f48d9cc9961bddce88372e8272cf87afd14b17301c54d`),
with application and native execution still pending. The expanded receipt
gate collected 375 tests and passed four focused controls on unchanged source;
its complete execution and final candidate composition remain pending. All
six milestones and seven release gates remain open.
