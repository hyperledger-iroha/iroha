# Original capture through validation and publication

Work location: `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
The liveness redesign remains active. This checkpoint follows build104 and the
separate merge resolution; it does not activate the production retained validator.

## Missing pre-marker continuation

Archive insertion preparation can refuse after actual execution has completed
and its State journals have been detached. `prepare_journals` already returns
that entire original `StagedCarrierCapture`, including an already prepared
provider plan when the reputation index is busy. The retained validation service
previously had no phase for it. Keeping this work inside a producer would require
another pending-owner store beside the existing candidate table; dropping it
would cause reexecution on a later dispatch.

`RetainedCarrier` now includes the original `Capturing` owner alongside validated,
decided and checkpointed execution. Its identity is read from the original
journals in every phase. `ready_commitment` returns `None` during capture. The
existing candidate descriptor is reserved before execution. The unfinished phase
retains the boxed capture already returned by archive preparation; retries keep
that same allocation until completion. Completed phases remain inline.

The service installs the executed owner into its candidate slot before invoking
the producer's explicit `resume` operation for an incomplete capture. A ready owner
skips that consuming operation and its temporary stack frames. Success and local
refusal both return the current owner to that same slot before any outward outcome. Identity is
checked again, and a separate `Deferred(LocalValidationRefusal)` result reaches
the existing local-validation path. It cannot become a deterministic rejection.
Even an adapter incorrectly reporting successful resume cannot create a success
marker while `ready_commitment` remains absent. A panic leaves the occupied
subject unavailable for reexecution; it remains a fail-stop condition.

Only successful original archive completion moves `Capturing` to `Validated`.
Later proposal rounds reuse the same subject owner, and failed marker sync still
retains the completed phase. No additional registry, scheduler, saved commitment,
State overlay reconstruction or compatibility path is introduced.

## Authenticate the target before derived writes

The private physical publisher previously authenticated the original Kura before
writing witness/archive artifacts, but it rejected a different State sharing that
Kura only when acquiring component writers. It now checks the existing captured
geometry's original State identity and exact block header immediately after
installation admission and Kura identity, before the first Kura lease probe.

This predicate uses the existing retained `Arc<BlockHashOwner>`; it does not
allocate, acquire a lock or mint publication authority. Failure returns the whole
original carrier with `ForeignTarget`. Later component predecessor, terminal
geometry/effects and storage-completion checks remain required. The terminal
adversarial geometry test now substitutes its private geometry after physical
acquisition so it continues to prove the final recheck separately.

## Validation scope

Build105 exposed a missing test-only `LocalValidationRefusal` import; that failed
build is retained as evidence. Build106 passed its build, 58 source-contract
controls and 173 of 175 focused runtime tests. Its failures were a stack overflow
in the long lifecycle regression and a terminal geometry fixture intercepted by
the new early target check. The debugger traced the overflow through selected
publication, source authentication and Kura finality decoding.

Build107 retains the original boxed capture across retries and checks readiness
before entering the consuming resume frame. The terminal fixture now substitutes
geometry after physical acquisition. The seven-package build passed in 137.71
seconds; all 160 publication/journal and 32 original-review runtime tests passed
on unchanged compiled inputs and an unchanged binary, with no stack override.
All 67 focused source-contract controls passed on 9,346 unchanged inputs.

The canonical gate then exposed three stale geometry expectations: the merged
`Observe` variant and match arms are test-only, but those exact source
expectations omitted `#[cfg(test)]`. Formal capture107b requires the actual
attributes and retains the production maintain/attest delegation. Its final scope
reruns 67 retained-owner controls and all 112 geometry/historical controls,
including removal and misplacement of the test gates. All 179 controls and the
canonical gate passed on their unchanged captures. A concurrent merge changed
23 Rust inputs after build107, including State fixture construction. Build108
and a fresh runtime run therefore replace the older binary in the final
qualification; the formal controls can be reused only when every captured
source still matches. The final commands, outcomes and source-manifest join are
recorded in `dist/sumeragi-main-work/validation108.json`.

The real capture regression reserves both archive predecessors and executes
signed genesis under an exact four-validator signed RS16 context. It then holds
a real reputation index reader through each BodyStore call. Two proposal rounds
retain the same capture allocation, six journal/source allocations and the
completed provider-plan allocation, return the actual index release wait and
original wake destination, and create neither success nor rejection markers.
Index release resumes that owner; a marker sync failure retains the completed
phase. Exact three-of-four finality, Kura checkpoint persistence and consuming
publication finish with exactly one execution. The original counted admission
objects remain until the published owner drops.

The target regression tries another State sharing the original Kura and an
incorrect captured header, with and without held Kura/State locks. It checks
unchanged witness staging/final directories and both archive namespaces, retained
original allocations, admission release, and eventual publication through the
original State. Separate tests reject marker authority for an incomplete owner
incorrectly reported as resumed, and prove that ready owners skip resumption
through marker failure, retry and cache reuse.

These counted test admissions establish lifetime behavior, not a production
resource policy. The new paths remain private integration prerequisites; these
checks do not establish live network liveness.

## Original producer and complete admission inputs

The selected carrier now lends its original producer to the consuming callback
alongside the current owned phase. The producer borrow lasts only for that call;
a returned retry cannot retain a borrowed State or Queue guard. Error and drop
restore the current phase to the original candidate slot, and success retains
the existing consumed tombstone. No replacement producer or dependency registry
is added.

The signed phase regression derives State, Kura and Queue from that producer.
While the original Queue observer is held, an independent empty Queue remains
available but cannot wake or progress the selected owner. Releasing the original
observer wakes its real release wait. The same checkpointed allocations then
survive a State writer refusal, explicit physical abort and final publication,
with one execution. The foreign State stays unchanged. This proves dependency
provenance; an outer Queue observer does not authorize lane retirement.

Journal admission exhaustively borrows every prepared candidate field before
projections: original ValidBlock, StateBlock, execution sources and witness,
shared context, execution commitment, Native manifest, deferred DA records,
publication events and both archive predecessors. Deferred records and events
expose their original vector capacities. The DA effect accessor also exhaustively
borrows its owner. Adding a retained field therefore requires reviewing the
admission mapping. Refusal and synchronous retry preserve the same block output,
context and event allocations, including deliberately spare event capacity; the
World regression inspects a real nonempty authenticated DA record before either
World or cache publication.

The first producer/admission build109 passed compilation, 212 source-contract
controls and the canonical gate, but the extended signed phase test overflowed
the default test stack; the other 202 runtime controls passed. The preserved
`dist/sumeragi-main-work/validation109.json` is failed evidence. LLDB shows the
same Kura proof/HeightContext decode reached by the older regression. Its caller
frame grew from 329,776 to 374,464 bytes after adding Queue and foreign-State
checks. The completed Queue refusal and foreign fixture construction are moved
into borrowed helper calls whose scratch frames end before physical decoding.
No stack override, replacement carrier allocation or lost assertion is used.

The final combined receipt is `dist/sumeragi-main-work/validation110.json`;
its checks must pass and its build, runtime and formal source manifests must agree
before this checkpoint is treated as qualified. Test admission objects still
establish lifetime behavior, not complete allocation accounting or production
enablement.

## Remaining live handoff

The existing lifecycle owner's `apply_service` slot should own
`RetainedBodyValidationService<V2ApplyService>` once its concrete funded producer
exists. The wrapper already owns the producer, so the original State/Queue pair
can move through recovery, launch and `V2IoHandle::spawn` without cloning the
service or introducing another registry. Validate must call the retained
BodyStore operation; both ordinary and lifecycle Decision Apply must select the
same confirmed owner. The consuming selection now supplies its original
producer alongside the phase. The live producer must use that access to derive
publication State and Queue from its own original service.

Create this service before cold marker revalidation and retain it through launch.
Kura finality alone does not imply that State applied the height: a pre-WSV crash
still needs exactly one newly executed retained owner. Already-applied recovery
must authenticate its canonical result-bearing block and repair durable
completion without creating journals or executing again.

Complete resource admission remains prerequisite. The journal callback now
exposes every retained candidate field; the concrete allocation policy still
needs accounting hooks through those original owners, including shared and
nested allocations. Earlier execution/event allocation, later full cold-tier snapshots,
geometry/archive capture, decision encoding, installation and actual delayed EBR
reclamation all require their real capacity owners. Borrowed post-execution
admission refusal cannot be retained across a wait; prior admission must cover
that operation. A body-wire cap, encoded-size estimate or unit reservation does
not supply this guarantee. Original Queue retirement, participant durability,
cold recovery and the one live Native cutover remain unfinished, followed by
full-workspace and unchanged four/seven-validator fault/restart/final-transaction
qualification. No liveness goal is closed.
