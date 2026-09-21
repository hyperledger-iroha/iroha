# Admitted Storage restoration

`Storage::try_from_snapshot_admitted` reconstructs exact current and undo trees
from borrowed snapshot entries, including explicit prior-absence tombstones.
Both trees remain private until every copy succeeds. Each edit reserves its
node demand and incoming nested payload copies from the destination's original
allocation pool. Capacity or planning refusal destroys the private destination
and preserves the original source for retry. The caller still authenticates
the source schema and fences snapshot acquisition against publication.

The composed first-release API stores that original pool in `Storage`.
`try_with_admitted_block` and `try_with_admitted_replacement` enclose both
physical writers in a higher-ranked callback. Refund wakeups remain deferred
through callback execution, publication and writer destruction. A callback
error abandons both private trees; guards cannot escape in its result. The
policy must retain the exact original pool and complete reservation. No
caller-supplied replacement pool or compatibility entry point remains.

Snapshot, history and JSON readers borrow the same current/undo owners in both
map modes. History iteration preserves canonical order and recorded absence
without copying payloads. The untracked projection API does not grant ordinary
cloning to prepaid storage.

The release gate keeps explicit executable selectors and checks their exact
source census, including the split allocator-backed Storage module and the
canonical Concread admission, checkpoint, pair, clear and deletion owners.
Missing, duplicate or stale names fail the offline gate; a small passing subset
cannot silently replace that declared coverage.

Three allocator-backed regressions exercise nonuniform key/value copies,
independent source and destination pools, exact current/undo bytes and pointers,
source destruction before destination reuse, and allocation-free history at
full capacity. Refusal and unwind are injected after all current entries and
a nonempty undo prefix have been copied. Planning and capacity refusal, policy
construction panic and actual payload-copy panic all preserve a healthy source
for retry. Nested payload witnesses require physical deallocation before credit
refund; native node charges are checked through total pool conservation, not
misrepresented as individually instrumented payload records.

Both complete MV layouts pass 268 runtime tests and one documentation test with
no compiler diagnostics. Their source/executable inventories retain all 265
merged regressions and the three new cases. A fresh Core unit-test build has no
diagnostics and all 58 selected admission, carrier-framing, certified-gossip and
completed-repair regressions pass. The default production Core library also
checks successfully with the same eight existing warning messages. All Rust
checks share 7,274 unchanged captured source/build inputs. The complete offline
release-gate suite passes 220 tests and 2,642 subtests; its exact 9,166-input
capture includes the added release selectors and source-census checks.

Checkpoint135 records the scoped artifacts in
`dist/sumeragi-main-work/validation135.json`. The earlier Python run exposed a
stale aggregate selector count and is retained alongside the correction. The
initial Core build overlapped the new integration tests; the subsequent stable
build supplies the qualified executable. The earlier interrupted merge build
is retained as a failure. No prior checkpoint's runtime pass is substituted for
the composed implementation.

Native mutex, publication and release-notification control allocations remain
outside constructor admission. Concrete World payload policies, detached
prepaid capture, State refusal propagation, decoding and aggregate execution
and restore work still require admission. Per-edit restoration does not make
the production Validate-to-Apply consumer ready. The live worker still calls
the scalar `validate_candidate`; the shared Native runner cutover and one
unchanged four/seven-validator fault candidate remain required. L1–L6 stay open.
