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
existing candidate descriptor contains the largest inline phase and is reserved
before execution.

The service installs the executed owner into its candidate slot before invoking
the producer's explicit `resume` operation. Success and local refusal both return
the current owner to that same slot before any outward outcome. Identity is
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
build is retained as evidence. Build106 compiles the corrected source and its
focused checks are in progress. The new real capture regression reserves both
archive predecessors, executes authority-signed genesis
under an exact four-validator signed RS16 context, then holds a real reputation
index reader through each BodyStore call. Two proposal rounds retain the same six
journal/source allocations and the completed provider-plan allocation, return the
actual index release wait and original wake destination, and create neither
success nor rejection markers. Index release resumes that owner; a marker sync
failure retains the completed phase; exact three-of-four finality, Kura checkpoint
persistence and consuming publication finish with exactly one execution. The
original counted admission objects remain until the published owner drops.

The target regression tries both another State sharing the original Kura and an
incorrect captured header, with and without held Kura/State locks. It checks
unchanged witness staging/final directories and both archive namespaces, retained
original allocations, admission release, and eventual publication through the
original State. Separate coverage refuses marker authority when an incomplete
test owner is incorrectly reported as resumed.

These counted test admissions establish lifetime behavior, not a production
resource policy. The new paths remain private integration prerequisites.

## Remaining live handoff

The existing lifecycle owner's `apply_service` slot should own
`RetainedBodyValidationService<V2ApplyService>` once its concrete funded producer
exists. The wrapper already owns the producer, so the original State/Queue pair
can move through recovery, launch and `V2IoHandle::spawn` without cloning the
service or introducing another registry. Validate must call the retained
BodyStore operation; both ordinary and lifecycle Decision Apply must select the
same confirmed owner. The consuming selection needs access to its original
producer alongside the phase so publication derives State and Queue from that
service, rather than from replacement caller arguments.

Create this service before cold marker revalidation and retain it through launch.
Kura finality alone does not imply that State applied the height: a pre-WSV crash
still needs exactly one newly executed retained owner. Already-applied recovery
must authenticate its canonical result-bearing block and repair durable
completion without creating journals or executing again.

Complete resource admission remains prerequisite. The current journal callback
exposes State and source inputs but not every retained field; the complete
ValidBlock/context/manifest/effects/events and archive owners must be inspectable
by admission. Earlier execution/event allocation, later full cold-tier snapshots,
geometry/archive capture, decision encoding, installation and actual delayed EBR
reclamation all require their real capacity owners. Borrowed post-execution
admission refusal cannot be retained across a wait; prior admission must cover
that operation. A body-wire cap, encoded-size estimate or unit reservation does
not supply this guarantee. Original Queue retirement, participant durability,
cold recovery and the one live Native cutover remain unfinished, followed by
full-workspace and unchanged four/seven-validator fault/restart/final-transaction
qualification. No liveness goal is closed.
