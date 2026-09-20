# Queue retirement and guarded geometry publication

This is bounded implementation and validation evidence for the `optimizations`
checkout at `/Users/takemiyamakoto/dev/iroha`. It does not close L1–L6 or qualify
production cutover. The dirty `iroha-docs` checkout was not edited.

## Implemented ownership changes

The actual Queue reservation-transition mutex now uses the shared publication
mutex, with a nonblocking retirement-observer acquisition returning the exact
failed mutex's release observation. All 22 previously inventoried blocking acquisitions and
the pending-work predicate remain owned by that same mutex. The observer retains
coordinator and participant work, durable reservations, commit/release barriers,
and incarnation identity. Six controls cover actual ownership, cancellation,
unwind, early release, independent queues, durable barriers and cross-route work.

Kura geometry application and catalog publication now share one implementation
under the original prune, canonical, geometry and sidecar guards. Existing
wrappers transfer those guards without releasing/reacquiring them. Canonical
recovery and capacity capture still precede geometry/sidecar acquisition; pending
GC still finishes before the sidecar guard. New tests retain all four guards
through FilesApplied, exact catalog retry and rollback-on-write-failure; foreign
catalog refusal preserves the original journal and physical ownership. The
existing retirement/recovery/GC tests remain intact.

Queue fixtures now choose their configured catalog before opening Kura and
constructing State. Policy-only changes use the original configured catalog.
Intentionally future/inactive fixtures use the existing explicit test projection
boundary. Canonical dataspace baselines and default routes agree, drain metadata
uses the real incarnation, and custom telemetry is attached once during State
construction. Daemon startup tests pass the frozen manifest baseline explicitly.
No production validation or test assertion was removed to admit these fixtures.
The event fixtures execute queued Network work after an explicit structural
predecessor: actual canonical body storage, the existing empty-block test metadata
commit, and the real DA index rebuild. Exact header/hash/wire readback and unchanged
predecessor bytes are asserted. This scoped fixture does not execute or authenticate
genesis and establishes no QC/finality or production publication claim.

The formal Rust item reader now locates function bodies after balanced parameter
lists, including destructuring, instead of mistaking parameter braces for the
body. Inventory authentication pins the canonical manifest mapping. The shared
fixture copier refuses production inventory authentication errors before creating
fixture files; a regression rejects the raw-file hash as a manifest pin.

## Retained failure evidence

Build34 passed compilation and 902 of 943 selected runtime tests: all 150 geometry
and the original 439 controls passed; 41 broader Queue tests failed. Their exact
failure list is retained in `dist/sumeragi-main-work/build34-queue-failures.json`.

The incoming merge changed 198 Rust/configuration inputs before build35. That
build retained unchanged inputs but failed four daemon test calls missing the new
manifest-baseline argument. A separately labelled early run on its emitted Core
artifact passed 19 of the prior 41 failures. The remaining 22 exposed the wrong
fixture construction order and four inconsistent default-dataspace routes.
Build35 is not a passing combined candidate. The initial formal inventory
failures are also retained; they exposed the canonical-mapping/raw-file hash
mistake rather than external source drift.

Formal35 passed 294 checks with all 8,133 captured inputs unchanged; the raw Git
index changed concurrently, so that run does not claim index immutability. The next
Rust changes were restricted to seven test/helper files. Full source, compiler,
artifact, failure and afterimage receipts remain under `dist/sumeragi-main-work`.

Build36 passed the combined five-crate build with 7,057 unchanged inputs and
954 of 962 selected runtime controls. Its eight failures comprised two initial
catalog fixture mismatches, two network transactions placed at genesis height,
and four UAID operations targeting Universal instead of the intended dataspace.
All original review regressions passed. Two incoming tests were included and
passed, but changed the old runner's expected prefix counts; their explicit
inventory audit is retained without rewriting that nonzero runner result.

Formal36 passed all 294 checks with its 8,133 inputs, HEAD and index unchanged.
The subsequent three-file change is restricted to Queue test code; its production
prefix and every formal owner are unchanged. Formal37 passed 39 focused checks
on those new bytes. Build37 passed compilation and 960 of 962 selected runtime
controls on unchanged Rust/configuration inputs and artifacts. HEAD changed externally during that run;
the raw audit preserves the mismatch and makes no Git-immutability claim.
The two failures showed that hash-only predecessor seeding in the two block-event fixtures lacks the actual
Kura block required for DA hydration. That incomplete fixture correction is
retained as failed evidence and does not qualify the two event tests.

## Queue/geometry and fixture validation through build38

The combined build37 compiled Core, Torii, test-network, Kagami and daemon library,
binary and selected integration harness targets with 7,057 unchanged Rust/config
inputs. Its 962 distinct tests passed 960: publication/composition 279, Native
custody 128, original review regressions 17, storage/startup 34, and Queue/geometry
502 of 504. Only the two predecessor-fixture tests failed. No tests were omitted;
both new incoming prefix members are included in the explicit expected counts.

The final change touches only `queue/routing_projection_resilience_tests.rs`.
Standalone Core build38 (`cargo test -p iroha_core --lib --no-run`, through the
repository fast-build wrapper) and both corrected event tests pass with unchanged
captured inputs and artifact. This is the standalone Core feature selection;
the 960 passing build37 tests were not rerun or relabelled as a fresh whole-suite
pass. The final source acceptance run passes three Native/geometry/historical
owner checks on 8,133 unchanged captured inputs. Production and formal inputs
match the earlier 294-check and 39-check runs; their results keep their original
source receipts. This establishes scoped component evidence, not network release
qualification or an unchanged-source full workspace pass.

All 33 Rust files covered by this work pass scoped rustfmt. The codec-retirement
guard passes. Workspace-format check36 retained three unrelated differences in
`iroha_test_network/tests/support/production_beacon_prepare.rs`,
`iroha_torii/src/lib.rs` and `irohad/src/external_software_signer.rs`; none overlaps
the formatted owned files. Exact build commands, source and executable hashes,
raw results and prior failed attempts remain under `dist/sumeragi-main-work`.

## Pending-hash lookup ownership (build39 and follow-up)

The Queue pending query now takes a coherent State view before borrowing its
live `txs` entry. The previous order retained a DashMap shard reader across
State acquisition. Together with Queue removal holding `push_remove_lock` while
waiting for that shard, a publisher holding the hash writer and then waiting
for the Queue lock would form a circular wait. That aggregate schedule is a
source-backed finding, not a reproduced production deadlock. Acquiring State
first removes this ordering seam and still checks live membership after any wait;
cloning a transaction before the wait would not establish that it remains queued.

The new deterministic regression owns the actual detached BlockHashes publication
writer, observes the query's exact view boundary, and runs actual Queue removal
while that writer is held. Removal completes, the query stays blocked until
writer abort, then returns false. State hashes and generation remain unchanged.
All workers are released/joined before failure assertions; there are no sleeps.
The existing committed-entry regression remains intact.

Build39 passes the combined five-crate/test-harness boundary with unchanged
7,057 captured Rust/config inputs. Its new concurrency test, existing pending
query test, both corrected event tests and stale-cache Torii test pass. The live
pending HTTP fixture fails before its pending assertions: an ordinary `Log` is
sent to the key-lifecycle ingress path and receives 409 instead of 202. This failed
result remains recorded. Formal39 passes 39 focused controls on 8,133 unchanged
captured inputs. This is a new production source epoch; the full 294-check run was on build36.

The Torii fixture now creates four real BLS authorities, matching canonical
contexts, separate durable QueuePlan journals,
and a real signed HTTP peer receiver. The unchanged public transaction handler
obtains local and peer receipts and returns 202; both queues retain the original
input, and the local status response retains the queued cache. No receipt or Queue
entry is injected. This test requires the production `connect` transport, included
in the default node API and the combined build's feature selection.

Build40 passed compilation and the same five controls, but its HTTP test reached
quorum and then received 503: the fixture used a current-thread Tokio runtime,
while the actual publication owner requires a multithread runtime. That failed
result is retained. Build41 changes the fixture to the required runtime and keeps
the Queue test notification within the same State-acquisition expression so it
follows an acquisition reorder.

The combined five-crate/test-harness build41 passes on 7,057 unchanged captured
Rust/config inputs. All six selected controls pass: the new pending-view lock
regression, its existing committed-entry control, both block-event fixtures, and
both stale/live pending-cache HTTP controls. Their exact artifact and source
receipts are in `dist/sumeragi-main-work/pending-view-controls41/summary.json`.
Formal41 passes 39 focused checks on 8,133 unchanged captured inputs, with HEAD
and index unchanged during that run. Formal39 retains its earlier source receipt.
Neither the earlier full 294-check selection nor the 962-test sweep was rerun on
this production revision. All 17 original review regressions also pass on the
fresh build41 Core artifact,
with unchanged captured inputs and executable hash: complete admitted inputs,
carrier framing/deferred custody, single-owner certified gossip, and authenticated
repair-temporary crash recovery. In total, 23 distinct targeted runtime controls
pass on this build. The latest source change outside fixtures is restricted to
pending-hash State-before-Queue ordering.

This correction does not make the complete retirement predicate nonblocking or
stabilize a negative result under the outer observer alone. The actual
push/remove and reservation owners still need a retained cut, their exact release
observations, and nonblocking component acquisition. The original State/Queue
pair must remain bound through worker handoff, detachment and retry. Production
cutover and unchanged four/seven-validator qualification remain open.

## Remaining connected work

The parallel carrier review found no further defect in the repaired batch-sizing
or deferred-custody path. Ingress still checks only the 1 MiB complete-input cap;
configuration and signed DA limits can make the enclosing carrier smaller. A
512 KiB block limit, 64 KiB autonomous headroom and an approximately 800 KiB input
is a source-derived regression candidate, not an executed configuration in this
checkpoint. Reproduce it and bind admission to complete framing/RS16 feasibility
before durable receipts as part of the existing open capacity-policy outcome.

The private aggregate publisher still refuses pending nonidentity geometry and
old participant-durability obligations. The shared guarded geometry operations
and Queue observer are prerequisites; they do not establish aggregate permission.
The consuming path must retain the Queue observer derived from the original
`V2ApplyService` State/Queue pair, prove the inner predicate lock order, complete
the correctly ordered Kura prelude and authenticated Native
history join, acquire original component writers, publish durable geometry, and
then expose exactly one State generation. A mutex release is not evidence that
pending Queue work disappeared.

Complete original Validate-to-Apply and pre-vote resource custody, reachable typed
local deferral, actual runner cutover and unchanged four/seven-validator
loss/reordering/backpressure/leader-failure/restart/final-transaction campaigns
before closing the liveness goals. Full workspace validation remains required.

## Superseded root-view paragraphs (verbatim)

These paragraphs are preserved exactly before updating the current root views.

### Status: parent-source evidence

```text
Both parents retain scoped validation evidence, preserved verbatim in the [merge source record](docs/history/2026-09-19/merge-source-publication-evidence.md). The local build33 source passed Core/Torii/test-network/Kagami/daemon compilation, 439 selected runtime controls and 179 source/inventory/mutation checks. The incoming source reported compiler checks, 166 carrier/publication controls, 181 Native/lane/body/source-guard controls, three admitted-input reader tests, 186 selected source-inventory/Taira tests with 1,300 subtests and two macOS-only skips, 22 native preparation controls and 24 QueuePlan publication controls. Its broader block/state selection passed 495 and failed 48; after repairing one merge-specific source guard, 47 failures remained classified by parent-source comparison rather than parent reruns. Its complete multilane structural gate reported eight pre-existing diagnostics. Earlier Linux and macOS source results remain historical. None of these parent results qualifies this merged candidate.
```

### Status: merge validation and remaining scope

```text
The conflict-resolution validation run passed Core library compilation with 11 warnings and all 239 selected native preparation/fixture source-contract tests across six independent shards; this scoped run does not qualify concurrent unrelated working-tree edits. Changed-file formatting, codec retirement, conflict/diff checks and historical archive verification pass. Workspace formatting still reports differences in three files unchanged by this merge. Full workspace tests and unchanged four/seven-validator qualification remain outstanding. Production native runner cutover, original Validate-to-Apply custody, pre-vote capacity policy, pending geometry/Queue retirement, participant durability and carrier-proof custody remain separate open outcomes. Native DA/pin/SCCP payload execution remains unsupported; its complete owners, broader NPoS evidence/penalty and sponsor/lease coverage, aggregate resource admission and native Windows namespace durability remain outstanding. No L1–L6 goal is complete.
```

### Roadmap: connected publication work

```text
The next Sumeragi cutover must carry the implemented consuming State publisher and exact retained output owner through the complete production Validate-to-Apply path, including cached/recovered markers and voting; preserve one original execution per exact subject. Keep original World/runtime journals, source groups, witness, result bytes, exact lease-bound checkpoint receipt and verified QC/Kura authority joined before visibility. Carry the pristine QueuePlan/NPoS control owner, authenticated applying context through suffix finalization and common AXT/DA/SCCP postchecks into the canonical validator while preserving State generation and complete carrier custody. Complete pending geometry/Queue retirement and participant durability under their real storage owners. Finish bounded pre-vote descriptor/capacity admission, capture, file-handle and installation resources, and reachable typed deferral. Capacity scanning must precede geometry/sidecar acquisition; public geometry/GC/history wrappers cannot run under the held joint lease. Preserve original route-directory custody during retirement and account for witness staging on physical retries. Keep obsolete State observations as typed refreshes, including a finalized height advance, and preserve independent beacon and key-lifecycle behavior. Complete and qualify Native DA/pin/SCCP owners, broader NPoS evidence/penalty and sponsor/lease coverage before opening the live Native header gate. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded history references, retaining exact source/finality/WSV joins and whole-bundle accounting. Defer physical GC behind release, snapshot/recovery pins and an instance-local deletion fence; qualify native Windows namespace durability separately. Connect the process-lived shared lane reducer to transport and canonical Decision application while retiring the old fresh-signing authority in one complete cutover. Preserve cross-route closure, immutable incarnation storage, drain/signing fences and historical native authority. Qualify one unchanged four/seven-validator fault/restart/final-transaction candidate and the full workspace before closing any liveness goal. Source-scoped evidence lives in [status](status.md); [superseded local plans](docs/history/2026-09-19/merge-source-publication-evidence.md) are preserved verbatim.
```

### Status: Queue/geometry prerequisites through build38

```text
The Queue retirement observer now acquires the actual reservation-transition mutex without blocking and observes that same mutex for retry; existing pending-work and incarnation checks retain their owner. Kura transition and catalog publication share one implementation under the original four guards, preserving recovery/capacity-before-geometry and GC-before-sidecar ordering. Queue fixtures establish their catalog before storage opens, and formal fixtures authenticate the canonical include-manifest pin before copying sources. These are scoped prerequisites for aggregate publication. The [dated record](docs/history/2026-09-19/queue-geometry-publication.md) preserves implementation, source identities and failed attempts.
```

### Status: validation through build38

```text
The combined Core/Torii/test-network/Kagami/daemon build37 passed; its 962-test sweep passed 960 and identified two event-fixture predecessor defects. The final one-file correction passes standalone Core test compilation and both affected tests; the other 960 results retain their build37 provenance. Formal36 passed 294 checks, the subsequent test-only change passed 39 focused checks, and final-source owner acceptance passed three; production/formal inputs are unchanged between these latter fixture revisions. Scoped formatting and the codec guard pass; workspace formatting retains three unrelated differences. Full workspace tests and unchanged four/seven-validator qualification remain outstanding. Production native runner cutover, original Validate-to-Apply custody, pre-vote capacity policy, pending geometry/Queue retirement, participant durability and carrier-proof custody remain separate open outcomes. Native DA/pin/SCCP payload execution remains unsupported; its complete owners, broader NPoS evidence/penalty and sponsor/lease coverage, aggregate resource admission and native Windows namespace durability remain outstanding. No L1–L6 goal is complete.
```


## Retained inner Queue ownership and supported carrier limits (build43)

The actual enqueue/removal and reservation mutexes now use the existing release-
notification wrapper, generalized to retain its protected value. The consuming
retirement cut acquires mutation then reservation without waiting, retaining the
original outer transition guard. On refusal it releases all attempted guards and
returns only the exact failed mutex’s observation. On success field order releases
reservation, mutation, then transition. Pending ownership still includes every
coordinator/participant route, exact incarnation, live reservation, durable barrier,
completed release and fail-stop condition. The original blocking predicate shares
the same helpers and preserves its prior release order. Later State/component
acquisition must remain try-only; no cut may cross an async wait or preparation.

Four runtime tests cover actual inner contention, unrelated/early release,
unwind/successor ownership, enqueue from a previously captured State view, and
exact durable barriers/fail-stop behavior. Build42 failed compilation because the
first concurrency fixture moved a thread-local State view into another thread;
all 25 diagnostics identified that same error. Its 7,057 captured inputs stayed
unchanged. Build43 corrects only that fixture: the worker captures and consumes
its own view, acknowledges capture before the cut, and all owners/channels release
before join/assertions. No Send implementation or unsafe workaround was added.

The combined five-crate/test-harness build43 and all 537 selected runtime controls
pass on unchanged 7,057 Rust/config inputs and unchanged executable hashes:
359 Queue, 150 geometry, eight State publication-lock, one small-carrier, 17
original review regressions and two Torii pending-cache HTTP controls. This reruns
the full affected Queue/geometry groups, including the earlier corrected fixtures;
it does not rerun the earlier 962-test selection or the whole workspace. Five additional
existing runtime controls also pass on the same Core artifact and unchanged inputs: exact
Kura record/cursor reuse, corrupt latest index, foreign/corrupt suffix rejection,
merge/beacon roots with exactly one commit, and invalid effects/post-seal drift
rejection. This gives 542 distinct runtime passes; the additional receipt is
`delegated-owner-controls43/summary.json`.

The new small-carrier test uses actual Sumeragi configuration validation with a
512 KiB block limit and 64 KiB autonomous headroom, canonical recommended RS16
geometry and a genuine four-authority approximately 800 KiB complete input.
That input passes the same worst-quorum sizing API used by Torii, but the actual
assembler refuses the configured carrier limit before signing on two attempts.
Original Kura bytes, State generation/parent and context remain unchanged. This
upgrades the earlier source-derived example to configuration/assembler unit
evidence. It does not execute Torii ingress, signed genesis or daemon startup.
Ingress must still bind one complete framing/RS16/native/topic feasibility owner
before any durable receipt, with exact retry and recovery authority retained.

The canonical full source-binding gate and 21 final targeted checks pass on 8,133 unchanged inputs. The preceding 83 focused checks also passed before the final delegated-owner binding corrections; the older full 294-check selection was not rerun.

The first focused binding run passed 72 of 74 checks. Two intentional wrong-mutex
labels escaped because the executable-code normalizer masks string literals. The
correction binds each label to the actual parsed producer expression while keeping
ordinary string/comment masking. The accompanying full gate reported 120 errors.
Forty-nine required-token spellings disagreed with the gate's raw-source consumer;
exact source spelling was restored without changing normalized executable relations
or literal values. A stale NPoS condition was bound to the current evidence/penalty
exclusion while retaining permitted beacon composition. The release-component
loader and actual component already shared one verified digest; only its stale
ledger declaration was corrected.

The next run passed all 83 controls on unchanged inputs, HEAD and index, while its
full gate retained five errors. Those bindings referred to superseded call sites.
Kura's snapshot now retains the exact authenticated record/current-cursor pair;
State's three consumers retain independently sealed composed merge/beacon events.
The final migration binds those defining helpers, their verified producer and
original count/bytes/root/finality checks, with 19 mutation controls and two
positives. A preliminary positive check caught three trailing-comma token mistakes;
its failed log is retained. The final 21 controls and full gate pass with zero
diagnostics and unchanged 8,133 inputs, HEAD and index. No Rust production, TLA,
configuration or invariant was changed by this binding correction. The 83-check
and 21-check runs are distinct formal source epochs; neither claims the older
full 294-control suite. Raw receipts and review details are under
`queue-retirement-cut-bindings/`, including the initial run, `final/`, and
`delegated-final/`.

The cut remains a prerequisite, not a live aggregate connection. Original service
State/Queue custody, complete admitted resources, authenticated old Native history,
durable nonidentity geometry/participant publication and one visibility interval
remain required. Live Validate-to-Apply/runner cutover, full workspace tests and
unchanged four/seven-validator fault/restart/final-transaction qualification remain
open. No L1–L6 goal is complete. Raw outcomes remain under
`dist/sumeragi-main-work`, including build42, build43 and
`queue-cut-controls43/summary.json`.

### Superseded root evidence at this checkpoint

The Queue retirement observer acquires the actual reservation-transition mutex without blocking and observes that mutex for retry; existing pending-work and incarnation checks retain their owner. Kura transition/catalog publication shares one implementation under its original four guards. Pending-hash queries now acquire State before the live Queue entry, removing a shard-reader/State-writer lock inversion while preserving live membership after a wait. A deterministic test proves real Queue removal completes while the actual State hash writer blocks that query. Queue and HTTP fixtures use canonical construction and authenticated admission; formal fixtures authenticate the canonical manifest pin. These are scoped prerequisites for aggregate publication. The [dated record](docs/history/2026-09-19/queue-geometry-publication.md) preserves implementation, exact validation scopes and failed attempts.

The latest combined Core/Torii/test-network/Kagami/daemon build41 passes, as do all 23 targeted runtime controls: 17 original review regressions and six pending-query/event/HTTP controls. Formal41 passes 39 focused checks on unchanged captured inputs. The corrected HTTP fixture uses actual signed peer admission, durable journals and the required multithread runtime. Earlier build37 passed 960 of 962 controls; both failed event fixtures pass after correction. The full 294-check result belongs to formal36, before the pending-query production fix; neither broad selection was rerun on the latest revision. Scoped formatting and the codec guard pass; workspace formatting retains unrelated differences. Full workspace tests and unchanged four/seven-validator qualification remain outstanding. Production native runner cutover, original Validate-to-Apply custody, pre-vote capacity policy, pending geometry/Queue retirement, participant durability and carrier-proof custody remain separate open outcomes. Native DA/pin/SCCP payload execution remains unsupported; its complete owners, broader NPoS evidence/penalty and sponsor/lease coverage, aggregate resource admission and native Windows namespace durability remain outstanding. No L1–L6 goal is complete.

The next Sumeragi cutover must carry the implemented consuming State publisher and exact retained output owner through the complete production Validate-to-Apply path, including cached/recovered markers and voting; preserve one original execution per exact subject. Keep original World/runtime journals, source groups, witness, result bytes, exact lease-bound checkpoint receipt and verified QC/Kura authority joined before visibility. Carry the pristine QueuePlan/NPoS control owner, authenticated applying context through suffix finalization and common AXT/DA/SCCP postchecks into the canonical validator while preserving State generation and complete carrier custody. Use the implemented nonblocking Queue observer and Kura operations that retain their original guards to complete pending geometry/Queue retirement and participant durability under their real storage owners; retain the exact State/Queue pair from V2ApplyService. Finish bounded pre-vote descriptor/capacity admission, capture, file-handle and installation resources, and reachable typed deferral. Capacity scanning must precede geometry/sidecar acquisition; public geometry/GC/history wrappers cannot run under the held joint lease. Preserve original route-directory custody during retirement and account for witness staging on physical retries. Keep obsolete State observations as typed refreshes, including a finalized height advance, and preserve independent beacon and key-lifecycle behavior. Complete and qualify Native DA/pin/SCCP owners, broader NPoS evidence/penalty and sponsor/lease coverage before opening the live Native header gate. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded history references, retaining exact source/finality/WSV joins and whole-bundle accounting. Defer physical GC behind release, snapshot/recovery pins and an instance-local deletion fence; qualify native Windows namespace durability separately. Connect the process-lived shared lane reducer to transport and canonical Decision application while retiring the old fresh-signing authority in one complete cutover. Preserve cross-route closure, immutable incarnation storage, drain/signing fences and historical native authority. Qualify one unchanged four/seven-validator fault/restart/final-transaction candidate and the full workspace before closing any liveness goal. Source-scoped evidence lives in [status](status.md); [superseded local plans](docs/history/2026-09-19/merge-source-publication-evidence.md) are preserved verbatim.

The earlier merge selection passed 11 metadata, 58 FASTPQ, 100 JavaScript and 51 source-helper tests; one expensive FASTPQ diagnostic remains ignored. The FASTPQ build has zero warnings. Formatting and retired-codec checks pass. The broader multilane audit still reports source-binding drift; its full gate and the merged workspace/network suites remain unqualified.
