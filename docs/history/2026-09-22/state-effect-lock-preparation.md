# State effect lock preparation

The direct State commit and retained carrier candidate now acquire the original
State effect/index locks before opening their visibility interval. One typed
inventory covers header, merge admission, DA commitment/confidential/receipt/
shard/pin indexes, relay cache, manifest/privacy registries and hydration status;
the original SCCP cache mutex is acquired last. Reader and writer contention
return a release observation from the actual blocking lock. A release is only a
retry hint: the caller must reacquire and authenticate its original operation.

The complete carrier owns an inert index slot before component preparation.
A failed late probe preserves the original carrier journals. Direct commit uses
the same lock inventory and effect consumers with synchronous acquisition. It
releases every acquired index prefix before blocking on the exact contended lock,
then retries the inventory. It must not surface ordinary contention through the
old Apply error path, which requires restart after a consuming commit failure.
Both physically release index locks before component/fence cleanup can notify. Displaced manifest/privacy/SCCP
allocations remain in the outer cleanup owner. Borrowing physical scopes enforce
that ordering on unwind, including panic during component publication or abort.
The direct commit's cached header now changes within the same generation as its
hash and membership publication.

The new reader/writer wrapper preserves parking-lot semantics and does not expose
a raw backend. It supports deferred same-source release custody. Its ordinary
standalone guard release guarantees only its own physical unlock; callers with
other locks must explicitly retain cleanup through those locks. It does not fund
native notification allocation or registration. Short direct merge-validation
reads coalesce into their original source batch. Cursor post-work retains the
final image captured through the original writer and acquires no fresh notifying
reader under State/Queue/Kura fences. Relay-cache hydration follows commit unlock
and retains its existing current-incarnation revalidation.

Review of the new shared acquisition order exposed a real relay-cache/World
cycle: candidate validation held the relay reader while opening a World reader,
whereas commit retained World publication locks while acquiring the relay writer.
The candidate now owns a snapshot of only the next contiguous relay per route
and releases the cache before World/Kura validation. A regression holds the real
World reader mutex and requires cache pruning to proceed while validation waits,
then checks that validation consumes the original selected envelope.

DA hydration also uses the new notifying indexes. Its six original notification
sources now live outside both the hydration fence and State write guard; all five
index replacements unlock before any batch can notify. Cursor persistence borrows
that same deferred reader owner and retains the captured journal for disk I/O.
The added controls probe actual sibling indexes and both fences on successful
hydration, rewind, failure and the generic reader/writer wrapper's unwind.

The same source-bound release custody now extends through manifest refresh,
relay admission, lifecycle reset and its nested geometry cursor work. Their
outer caller retains the actual notification sources until the lifecycle,
State and visibility fences have released. Replay geometry explicitly receives
no live cursor-publication authority. Certified-lane snapshots and persistence
authority derivation retain their cursor reader release outside the State fence;
merge-cache repair retains its admission release through both State unlock and
the completed visibility interval. None of these callbacks can substitute for a
fresh authenticated retry.

Drain classification reuses its captured lifecycle and the same original index
release owners through pending-admission validation. Its coherent State view
borrows those owners for header, manifest and SCCP reads. Ordinary and replacement
block acquisition now capture the immutable manifest baseline and SCCP cache
before opening native writers. The final generation check still authenticates
the manifest projection; the SCCP validator checks the acquired World's exact
wire against its private cache copy, including replacement undo state.

Replacement still admits its hash successor and acquires the original State
writers before changing live DA projections. Its DA rewind now installs the
six original index release batches and State-write notification in that acquired
owner before entering the fallible rebuild. These move through the executing
State block and remain outside partial journal capture. Success delivers them
after detachment; refusal, abandonment and unwind first release every writer.
The rewind method uses the acquired owner's original State, rather than accepting
a separately selected target. No replacement ordering or allocation pool changes.

Committed drain metadata validation now receives the same retained lifecycle
index owner as its caller. Its manifest reader therefore cannot notify under
the enclosing commit/lifecycle/State fences. The new regression uses a real
four-validator staged drain intent and covers success and exact staged-policy
refusal. State and Kura are constructed together with the intended immutable
catalog; the existing cold-window fixture now uses that constructor too.

## Validation scope

Current Core qualification passes all 778 selected runtime controls, including all
58 Core regressions mapped to the original five review findings and the committed
drain regression. Independent verification joins the build, copied executable,
every result/log and the current 7,547 Rust/build inputs. The seven-package
`--tests` compilation also passes on those same inputs with no errors or Core
warnings (127 warnings in other crates). A fresh Torii build and all 21
deadline/capacity controls pass on the same inputs, including all five original
persistence-deadline controls. This is scoped evidence, not full-workspace or
network qualification.

All 230 lifecycle/acquisition/drain formal controls passed in one fresh
copied-source run with no failures, errors or skips. Independent verification
joins all 1,719 inputs at that cut, exact selectors, XML and log hashes. All
1,624 previous selector IDs are preserved; only the three drain mutations were
added, producing a 1,627-control inventory. This is not execution of the entire
inventory. The drain change updates one caller ledger row and adds its defining
helper, preserving all 1,785 other rows. These receipts are under
`generation156-formal/drain-metadata/`; the earlier scope remains below.

The subsequent full structural run, `generation156-formal/canonical8/`, failed
on unchanged 12,287 inputs: two raw source-binding strings no longer matched
Rustfmt layout, producing four errors across the ledger and Native checker.
Only the two expected strings and their two existing ledger rows were aligned;
the checker algorithm and Rust were unchanged. Normalized old/new calls are
identical, including receiver, target height, registry, privacy, generation and
release owner. The preserved 230-control run checked normalized ownership
relations and did not exercise these raw token comparisons.

A separate 126-control follow-up includes 95 overlapping lifecycle/rewind/owner
controls, all 24 existing raw Native prepublication controls, and seven new raw
ledger acceptance/substitution controls. Its execution and independent source,
ledger, selector and log verification are recorded under
`generation156-formal/final-token-alignment/`. Its copied closure expands to
1,896 inputs to cover every Native model production row. The 230 and 126
selections contain 261 distinct controls across their separate source scopes;
they are not one unchanged run or execution of the entire formal inventory.

The 227 focused lifecycle/acquisition formal controls pass across an unchanged
219-pass/8-failure initial run and a fresh 96-pass follow-up (93 new mutations and
three positives, with 88 overlapping initial passes). The eight failures were
test defects: two early-cleanup mutations duplicated a moved owner, two expected
the wrong ordered-relation diagnostic, and four searched quoted Rust for the word
`digest` instead of checking the structured diagnostic. Corrected controls require
the exact defining owner and executable relation/order; no checker or ledger rule
was weakened. Independent verification joins all copied inputs, selectors, XML
and log hashes to the final 1,719-file closure. All 1,624 current selector IDs are
preserved; this is not an execution of that entire inventory. These receipts are
under `generation156-formal/lifecycle-index/` and precede the drain correction.

The preceding complete-participant
candidate's 649 Core and 355 MV controls do not qualify these new source changes.
The initial effect candidate passed 672 Core controls on unchanged 7,540 Rust/build
inputs. Its downstream seven-package test-target check then found a fixture cfg
mismatch; the corrected check passed. Subsequent relay/hydration fixes require
fresh qualification and are not covered by those earlier results. The 545 distinct
formal controls passed across an immutable 537-pass/8-fixture-error run and a
10-pass follow-up (eight corrected cases plus two overlapping positives).
The original QueuePlan ledger acceptance regression also passes. These are
scoped compilation, runtime and source-contract results, not network qualification.
The original five review defects have a source audit: all 58 Core controls remain
in the current 778-test execution. The older 21-test Torii receipt is superseded
by the fresh current-source Torii build2/runtime2 qualification above.

The expanded 730-control run completed on unchanged source with 724 passes and
six failures. All six also fail in the retained earlier executable: their shared
relay-commit fixture staged transaction membership but never finalized the
reserved block-hash tip, so commit refused before any relay assertion ran. The
fixture now uses the existing strict helper that stages its exact header hash.
A fresh Core test build and all eight focused relay controls (the six original
cases plus the two cache/World regressions) pass. These earlier receipts remain
separate from the current 778-control qualification above.

The next immutable Core test build passed with no compiler diagnostics. Its
36-control diagnostic run passed 35 controls; the new compatible-manifest fixture
failed its catalog-binding assertion before calling the behavior under test.
The fixture now uses its actual immutable catalog and manifest. Its corrected
control and the additional acquisition/rewind custody work pass in the fresh
build and joined runtime result above. The failed receipt is retained unchanged.

After those changes, another immutable build passed with no compiler diagnostics.
Its 44-control diagnostic run passed 43 controls, including the corrected manifest,
acquisition snapshot, SCCP replacement/cache and carrier-capture tests. The remaining
rewind fixture called direct commit after raw field construction, bypassing the
replacement constructor's required AXT initialization. That fixture now uses the
actual constructor. Both rewind controls pass in the final diagnostic and in the
current full 778-control selection. The earlier 43/44 receipt remains unchanged.

Current receipts under `dist/sumeragi-main-work/generation156-core/` are
`effect-publication-core-build15`, `effect-publication-runtime4`,
`effect-publication-torii-build2`, `effect-publication-torii-runtime2`, and
`effect-publication-consumer-check5`, with independent runtime and consumer
verification JSON beside them. The final repository gate and hygiene results are
recorded separately in `generation156-formal/canonical9/result.json` and
`generation156-core/effect-publication-final-hygiene2/summary.json`; their status
must be read from those receipts rather than inferred from runtime results.

The drain regression first stopped at an invalid test constructor; the existing
cold-window test had the same immutable-catalog mismatch. Their setups now use
the canonical configured constructor, without changing the assertions. On those
corrected fixtures, build14 runs 14 existing drain controls successfully and
reproduces the new failure: the manifest waiter is called once while all three
committing fences remain held. Build15 changes only the three production lines
that forward and borrow the original release owner and use its manifest reader.
The same 15 controls then pass, including successful validation and the exact
policy-mismatch refusal. The complete before/after Rust input comparison confirms
that only `state.rs` changed between the failing and passing executions. These
receipts and exact patches are in `drain-metadata-release-regression/`. The full
post-correction runtime and formal selections now pass as scoped above.

## Open boundary

The direct commit API remains synchronous until the funded retained Apply cutover.
Index insertion/pruning and pin-cache recovery still allocate or retire payloads;
complete resource admission and private successor projections remain required.
Process observability and post-generation persistence still have their own locks
and allocations. Membership history still grows its unadmitted table during
publication. The candidate does not activate retained Validate-to-Apply or Native
production ingress, and does not close any L1–L6 goal. Unchanged four-/seven-peer
fault, restart and final-transaction qualification remains required.
