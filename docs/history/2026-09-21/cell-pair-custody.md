# Cell pair acquisition and release

The ordinary and charged `Cell` paths now retain one owner for their actual
current/undo writers from acquisition through abandonment, detachment and the
publication-lock check. The same native EBR clone engine serves both the direct
native API and the acquired phase; no compatibility path or second allocator is
introduced.

Previously, `acquire_charged_writers` cloned undo before acquiring current.
Current clone failure could notify a retry while undo remained locked; undo
destruction then reclaimed its private payload under that lock. Complete Block
and CurrentReplacement abandonment and explicit detachment also released and
notified the two writers sequentially. A known poisoned current lock was checked
only after constructing undo.

`EbrCellWriterAcquisition` retains the physical mutex, including poison, before
admission or cloning. Its consuming clone method returns that same acquisition
and the original private generation separately. A callee panic therefore still
releases and poisons its actual mutex even when the caller catches the panic.
Admission refusal and poisoned attachment return their original owners. Native
writer destruction releases its own physical mutex before payload reclamation.

MV acquires both original locks before either clone. An incomplete pair retains
completed private generations and unused charges until both physical guards
release. The complete `CellWriters` owner keeps the same joint release boundary
for Block and CurrentReplacement abandonment and detachment. Direct publication
retains it until the fallible publication-lock acquisition succeeds. Known undo
poison fails before waiting for current. Both original notification sources
observe actual poison after physical release.

## Evidence and limits

Checkpoint 150 was revalidated after an external merge introduced the compact
World owner. Its joined receipt, `dist/sumeragi-main-work/validation150.json`,
binds 1,144 Rust tests, 528 selected formal/source checks, and the release-checker
suite to unchanged merged source. That receipt precedes this Cell change.

The checkpoint 151 red run added a raw, non-constructing acquisition probe and
eight tests while leaving MV construction unchanged. Seven assertions failed:
they exposed held-sibling notification/reclamation, sequential detachment,
cloning before known current poison, and the missing joint acquisition before
the first clone. The first-clone and success controls also enforce the new
two-lock construction invariant; their old ordering failures alone do not prove
a lost notification. Known undo poison already passed. An earlier probe build
failed the unused-field lint and is retained as build evidence, not a runtime
counterexample.

The implemented change passes all 466 Concread and 202 MV library tests on its
captured compiler inputs, including nine Cell regressions and four native
acquisition/reclamation tests. The additional Cell regression exercises poisoned
publication-lock refusal in ordinary/replacement commit and CurrentReplacement.
The native tests check exact allocation identity, retained admission/poison
refusals, consumed clone panic, and unlocked payload/charge destruction.
Further compiler, consumer, formal and release checks belong to the joined
checkpoint 151 receipt only after they complete successfully.

Arbitrary partial `Clone` failure still conservatively retains its allocation
charge: it cannot prove that the failed clone freed every nested allocation.
This change does not establish complete State/World/runtime acquisition custody
or total resource admission. All six liveness goals remain active; the retained
Validate/Apply production cutover and unchanged real four/seven-validator fault,
restart and final-transaction qualification remain outstanding.
