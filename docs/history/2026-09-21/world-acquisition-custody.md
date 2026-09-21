# World acquisition and abandonment custody

Ordinary and replacement World construction retain every inert field slot before
initializing the first field. The existing overlay inventory generates partial
acquisition and complete-block release. The nested TriggerSet retains its ten
original storage slots through the same boundary. Standalone MV construction
uses the same underlying acquisition implementation.

Previously a later constructor could unwind and notify while an earlier World
field remained locked. Complete World drop also released and reclaimed fields
one at a time. Two actual World tests reproduced ordinary and replacement drop:
the parameters callback observed the peers writers still physically Busy.
These callbacks only recorded outcomes and retained original probe cleanup;
assertions ran after aggregate drop. That counterexample establishes early
notification, not a measured production deadlock by itself.

Each caller-owned slot retains raw guards, completed original generations and
unused charges before the next fallible operation. On abandonment, the enclosing
owner first releases every acquired writer. Slot destruction then reclaims its
original payloads and credits and emits original release notifications. Complete
World and TriggerSet owners perform the same two-pass release. Explicit terminal
retirement permits an enclosing owner to defer cleanup further.

Native phase transitions check the original notification source and retain an
actual callee-unwind release in the caller's batch. Failed B+tree cursors can
unlock into an opaque cleanup-only owner; that owner cannot read, edit, reattach,
or publish. No replacement release source or global suppression scope is used.
A payload cleanup panic after physical unlock no longer poisons an untouched
sibling writer; actual writer unwind still reports physical mutex poison.

## Validation scope

Checkpoint 152 retains the two failing original World controls, development
build diagnostics and final candidate validation under
`dist/sumeragi-main-work/generation152-core`. Its joined validation receipt is
required before reporting the complete candidate's pass counts. The new tests
cover World and TriggerSet ordinary/replacement cleanup, deferred explicit World
retirement, canonical WorldBlock JSON, caller-owned MV partial clone/replacement
unwind, known poison, terminal reuse refusal and native phase-source custody.
The existing default 2 MiB stack controls remain required.

The first development integration check found a macro item delimiter error. The
first Core test build found three existing fixture field moves that needed the
explicit consuming World transfer. The first MV run exposed the obsolete
cleanup-induced sibling-poison expectation described above. These failed runs
remain evidence, not passing qualification. The first source-stable integrated
Core run exposed 75 ordinary-stack overflows during real validation, despite
passing the direct World controls. Debugger evidence locates the excess in the
new acquisition slot aggregate: mutually exclusive raw, converted and complete
owners occupied separate storage simultaneously. Qualification must include the
complete affected Core selection after compacting those original owner phases;
increasing thread stacks is not an accepted correction. The compact candidate
represents these phases as alternatives in the original slot, with notification
batches outside the phase and destroyed last. It adds no allocation and retains
the same terminal retirement and poison checks. World also fills the inert slots and materializes the completed fields in
separate borrowed closure calls. The caller keeps the same acquisition owner
through both stages; constructor and final-move temporaries do not overlap
during deep native construction or final TriggerSet transfer. The complete affected runtime selection remains
the acceptance test for ordinary-stack behavior.

## Remaining boundaries

This change covers acquisition and abandonment. Existing consuming World and
TriggerSet capture/commit still extract their fields; keeping aggregate cleanup
through those transitions remains open. Enclosing State/runtime acquisition,
complete execution/restore resource admission, retained production Validate/Apply
cutover and unchanged real four/seven-validator fault/restart/final-transaction
qualification remain required. Ordinary user code called by Clone or a mutation
retains its operation semantics; this does not promise that arbitrary user code
runs without locks. Partial failed Clone still retains its conservative charge.
All six liveness goals remain active.
