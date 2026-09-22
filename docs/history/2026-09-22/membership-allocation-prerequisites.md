# Membership allocation prerequisites

This change on `optimizations` adds two concrete allocation boundaries needed by
pre-execution membership preparation. It does not yet replace the production
membership DashMap, fund the complete State, or complete the Native cutover.

`mv::allocation::ChargedBuffer<T: Copy>` replaces the byte-only owner and is used
by actual snapshot input reads as `ChargedBuffer<u8>`. It reserves the exact typed
layout before allocation, retains the original charge until physical free, and
exposes bounded append, in-place ordering and prefix truncation. Logical capacity
also applies to zero-sized elements. Bitwise append never calls a custom Clone.
Referenced storage, comparison code and surrounding control owners are separate
admission obligations; no compatibility alias or second implementation remains.

The original prepaid B+tree owned generation now reports a checked necessary
allocation floor for fixed Copy payloads. Its constructor retains original base
node counts; the explicit diagnostic includes all still-owned private nodes and
current tracking buffers, including privately retired nodes. It never subtracts
retirement entries from a retained base or scans on every normal edit. Stale
successor chains and other pool owners are excluded: this is a lower bound,
not publication authority or a promise that a shared pool will eventually fit.

Expanded snapshot qualification found six controls failing before their intended
assertions. Their shared SCCP fixture persisted its first block before creating
authenticated initial lane storage. All six failures reproduce on the preceding
frozen executable. The fixture now follows production startup order and seeds the
same exact latest-header cache after persistence. Signed block/archive/finality
writes and hostile-input/refund assertions are preserved; Kura's rejection of
missing committed physical storage remains unchanged.

## Validation scope

Generation157 evidence is under `dist/sumeragi-main-work/`. The original six byte
allocator controls remain, alongside five typed-buffer controls. MV passes all
360 controls for each tree layout and Concread passes 479 default/477 skinny
controls (two existing default-only split tests explain the difference). The five
new owned-floor controls witness actual allocation/free layouts, retained readers,
checkpoint rollback/apply, stale predecessors and caught-copy-panic rejection.
The canonical structural gate passes on its captured inputs; it does not assert
production trace extraction.

The initial expanded Core run is preserved as 877/884 passes: six shared-fixture
failures and one two-second lock-order watchdog timeout. The same lock-order test
passed unchanged twice on each old/current executable afterward; its timeout and
assertions were not weakened. The corrected-fixture executable passes all 884
selected controls, including every original 778 Core control and all 106 snapshot
controls, on unchanged inputs and binary. Subsequent consumer qualification is
joined with the [partial-write repair correction](native-repair-partial-write.md);
it must preserve the intervening source delta and these initial failures.

## Remaining production integration

Prepare the canonical previous-tip hash batch and private history successor
before acquiring World writers. Retain the exact predecessor, partial batch and
original pool across refusal; install caller-owned attachment slots before later
acquisition can fail. Returning new capacity errors only from committed-state
publication would still force restart recovery. A local refund scope cannot
return physical writers or wake callbacks while sibling World locks remain held.

A charged history reader also introduces a new refund/notification obligation at
view destruction. Preserve coherent tip/history cuts and height filtering, publish
history before its hot tip, and carry original reader releases outside State
fences. Fresh State, normal/Emergency snapshot seeds and isolated replay must use
the same configured pool before allocation. A finite pool cannot guarantee
unlimited resident history: irreducible demand must never become a fictitious
reader-release retry. Complete membership funding/restore, retained production
cutover and same-candidate four/seven-validator fault/restart/final-transaction
qualification remain open. All six liveness goals remain active.
