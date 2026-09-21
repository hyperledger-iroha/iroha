# Borrowed checkpoints of the original admitted map writer

Checkpoint124 adds private transaction rollback to the existing prepaid map
engine. Work stays in `/Users/takemiyamakoto/dev/iroha` on `optimizations`.
It does not activate funded production Storage or close L1–L6.

## Ownership and rollback

An exclusive borrowed checkpoint saves the original root, length, private
transaction tag and initialized bookkeeping cuts. Its checked fresh tag forces
new edits to copy parent nodes, including unpublished parent nodes. Rust reborrows
enforce nested LIFO resolution and prevent parent publication, detachment or
mutation while a child or its borrowed values/snapshot remain live.

The first growth of each tracking buffer retains the exact parent allocation.
Nested apply transfers both saved buffer owners to the parent before arbitrary
charge destructors run. It retains the newest private tag and does not publish.
Abort restores the original root, tag, length and buffers, discards child retirement
pointers, and frees only the new-node suffix. It removes each pointer before
calling its nonrecursive destructor; a cleanup guard drains the remaining suffix
if destruction unwinds. Abort needs neither allocation nor additional credit,
even with an exhausted budget. It never recursively frees a child root that may
still reference original parent nodes.

Closed insertion is shared by detached, held-writer and checkpoint operations.
A logical failed-edit flag covers mutation and cleanup, so catching a panic before
the physical writer lock unwinds cannot publish a partially changed cursor.
Reads, further edits, detach and commit refuse that cursor. Public publication
checks failure before consuming the original shells. The entire physical writer
and checkpoint lifetime remains inside the original budget's refund-notification
deferral scope. Payload destructor double panic keeps ordinary Rust fail-stop
semantics; conservative charge retention is not reported as physical reclamation.

## Scoped validation

Five new MV allocator controls cover full-budget abort with multiple buffer
growths; nested apply, outer abort and sibling publication; exact input return on
capacity refusal; caught edit panic; and tracking-buffer refund panic. The nested
control also exercises an admitted edit directly under the original held writer.
Four internal controls check exact metadata and buffer addresses/capacities,
generation exhaustion, nested buffer transfer and safe sibling-tag reuse.
Tracking pop/truncate controls preserve initialized ownership during unwind.

The final focused normal and native AddressSanitizer runs (ordinary and skinny
node geometry) each pass 186 tests: 137 MV library, 18 admitted-map, 13 linear,
5 EBR and 13 map-generation controls. Strict MV library/test Clippy passes.
The public API controls compile and execute four positive programs and reject
32 invalid capabilities with their intended diagnostics, including parent
commit/detach during a live child and escaping borrowed values/snapshots.

The vendor matrix passes 297 default, 295 skinny, 295 release, 326 async and
297 AddressSanitizer controls, with no failures or ignored cases. All 63 vendor
and harness source hashes still match. Those runs captured HEAD8660; an external
commit advanced the same branch to HEAD9ad43 without changing these inputs.
`vendor-post-commit-join.json` records matching source contents and differing Git
metadata explicitly. These runs were not falsely relabeled or repeated.

The final captures, Core library check, canonical/review-ledger gate, formatting,
archive and codec guards are joined by `dist/sumeragi-main-work/validation124.json`.
The initial test compile failure (unresolved drain-buffer type inference) and the
strict Clippy test-style failure remain in their original logs; both were fixed
without weakening ownership assertions. Historical checkpoint123/122 receipts
retain their own source and runtime scope. No new Core/Torii runtime or real
network run is claimed by this checkpoint.

## Next production boundary

Actual `Storage` still uses an Untracked current tree and a separate
`EbrCell<std::BTreeMap<K, Option<V>>>` undo owner. Replace that opaque standard-map
allocation boundary with the same typed tree owner through block, detached,
prepared-publication, snapshot and history paths. Preserve exact first preimages,
no-op and missing-key touches, replacement history and original two-writer
publication identity. Then jointly admit current edits and first preimages and
connect child checkpoints to the production transaction owner.

Concrete model payload policies, admitted removal/iterator/transaction storage,
native lock/runtime allocations, aggregate configured State budgeting, complete
Validate-to-Apply cutover and unchanged-source four/seven-validator failure,
restart and final-transaction qualification remain required.
