# Prepaid writer and node custody

This checkpoint stays in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
It implements original writer input and concrete B+tree node charge custody.
It does not activate the retained validator or claim complete map/State funding.

## Construction and handoff

`LinCowCellCapable` consumes an explicit associated `WriterInput`.
`WriterAdmission` joins that original input to cursor/reader shell charges while
the original writer lock is held. Both admitted shells are allocated before the
constructor consumes its input. Refusal, detachment, contention and reattachment
retain original ownership without a second admission or constructor call.
Unit convenience entry points require unit input explicitly; non-unit untracked
synchronous/asynchronous callers supply input under the same lock. The asynchronous
cell has no new charged-shell interface and remains outside Iroha's enabled features.

An input constructor may consume and destroy nested charged ownership before it
unwinds out of the cell. Therefore the complete synchronous funded operation must
use its original budget's notification-deferral scope; generic field drop ordering
cannot defer arbitrary nested destructors. Allocator-observed regressions retain
an actual input Box and its charge through the original cursor, and verify physical
free before refund and unlocked notification on abort/constructor panic.

## Actual node ownership

The existing B+tree implementation now carries a static charge type through raw
node pointers, concrete leaf/branch allocations and pointer-bearing node states.
Every allocation site—empty leaf, split leaf, branch, leaf clone and branch
clone—requests its exact padded layout from an explicit funding owner first.
Charges live inside the original allocation, including their own padding cost;
zero-sized `Untracked` preserves the existing ordinary cache-size checks.

One RAII owner covers the initialized prefix throughout cloning and fallible
verification. Reclamation reconstructs that same `Box<CachePadded<...>>`, extracts
the charge without releasing it, destroys/deallocates the node, then drops the
charge. Payload destructor unwind conservatively retains capacity. Branch children
remain borrowed until the existing tree/generation owner reclaims them. There is
no erased allocator registry, default charge or second B+tree engine.

Tests observe actual `System.dealloc` and exact pointer/layout identity. They cover
all key/value clone failure positions, branch separator cloning, three leaf split
directions, full-tree teardown, constructor comparison unwind and payload Drop
failure. The parallel audit also found the old raw-node `Sync` implementation
required only `K: Send`; it now requires `K: Sync`. Restoring the old bound makes
the new compile-time negative assertion fail with its intended ambiguity.

## Evidence and unfinished integration

`dist/sumeragi-main-work/validation115.json` is the final source/executable join.
Its component receipts distinguish the root workspace build/runtime tests from
the isolated manifest that exercises the actual vendor source, including native
sanitizer and skinny-node configurations. Failed initial compile attempts and the
old-bound counterexample remain in `generation115/`; they are not passing evidence.

All current map cursors still explicitly provide `Untracked`. A closed map edit
must next plan one complete bounded operation under the original root, pass its
prepaid provider through the cursor, and replace growing tracking vectors with
charged fixed-capacity buffers. The original retirement buffer must move to its
reader generation until actual free. Ordinary `Clone`, unrestricted mutation,
initial map/root controls and MV undo allocation remain unfunded. Complete
Validate/Apply and retirement integration plus unchanged four/seven-validator fault,
restart and final-transaction qualification remain open. No liveness goal is closed.
