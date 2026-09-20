# Original linear-cell allocation custody

All edits and validation use `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`. This is a prerequisite for complete retained-validation
admission. It does not activate the retained validator in the live worker.

## Actual owners and release order

The synchronous linear cell now accepts an opaque move-only charge for each
original cursor and reader shell. Admission sees the exact concrete allocation
layouts, reserves both writer shells before either allocation, and constructs
the cursor only after both shells exist. Publication initializes that same
reserved reader allocation; detach, busy/changed refusal and adoption retain
the original cursor, base generation and charges. A charged cell has no
untracked `write` or `try_write` convenience path. Production maps still use
the explicit `Untracked` mode until whole-operation admission is implemented.

The private strong-only owner uses a concrete Box-allocated control block with
an atomic reference count. It exposes neither weak references nor raw ownership.
The last reference joins previous releases, moves out the payload and charge,
and frees that exact block. Payload destruction precedes charge refund. This
avoids guessing the standard library's private Arc layout or treating entry
into a payload destructor as proof that its control block was deallocated.
If payload destruction panics, its charge remains conservatively retained.
These are control-block charges; nested payload allocations remain separate.

Commit retains the cursor charge until the new reader is visible and both
publication locks are released. Abort unlocks before destroying private work,
while retaining its original base, as explicit detach followed by abort already
does. Construction unwind releases the poisoned writer before either shell
refund. This order matters because a refund can synchronously invoke a waiter
that reenters the same cell. A panic in private destruction after unlocking does
not poison an already-abandoned writer; a panic while constructing or holding
the original writer still does.

Both permanent cell mutexes initialize at construction. MV release notification
state initializes its native mutex at construction, and each pending waiter
initializes its native mutex before publication. On platforms with lazy mutex
allocation, a first refusal or first refund must not need another allocation.

Final B+tree destruction now walks original child pointers with a fixed stack,
instead of collecting all nodes in a heap vector. A committed balanced tree's
minimum branch fanout and representable entry count bound that stack by
`usize::BITS + 1`. Each actual node is freed once after its children. Scalar
teardown tests include empty, leaf and multilevel maps, retained reader chains,
detached writers surviving their source map, and a stale detached base retaining
a newer committed root. They require zero teardown allocations and balanced
actual allocation/free counts and layout bytes over the complete fixture.

## Evidence

The local receipts are under `dist/sumeragi-main-work/generation113`; immutable
Core executable captures use `publication113` and `review-current113`. The
aggregate source/executable join is `dist/sumeragi-main-work/validation113.json`.
That join requires the complete eight-package build, exact runtime executables,
source inventories, branch, HEAD and index to agree. The final receipt is
authoritative for the combined outcome; historical passes are kept separately.

The frozen workspace artifacts pass all 153 MV controls: 127 library tests,
five EBR allocation tests, thirteen map ownership tests and eight charged-shell
tests. The actual vendor source passes 247 production-feature tests, 121 B+tree
tests with smaller nodes, 246 optimized tests and 247 native AddressSanitizer
tests. One debug-only constructor test does not exist in the optimized build;
these overlapping configurations are not distinct-test totals. One positive and
two negative compiler controls establish the charged writer API boundary against
the actual workspace dependency artifact. All 212 selected formal ownership and
geometry controls pass on 9,360 unchanged source inputs. The canonical gate also
runs the original pending-membership ledger regression explicitly.

The Core selection retains 171 publication/World controls and all 32 original
review regressions, including complete input decoding, exact carrier framing,
single-copy certified gossip and completed-repair temporary recovery. Full
workspace runtime and real-network liveness results are not inferred from this
selection.

The eight allocator-custody regressions observe concrete allocation layouts and
record successful `System.dealloc` before each original charge is returned.
They cover complete reservation refusal, busy and stale original writers,
source-cell destruction, constructor unwind, payload/charge destructor panic,
concurrent last-reader release, and synchronous reentry from refunds after
abort, unwind and publication. They use the actual MV allocation credit pool.
The private reference-count test also stresses concurrent clones and unique
mutation. Source contracts and the original review regressions remain separate
checks, not substitutes for these runtime allocation witnesses.

The first allocator run exposed the original writer's lazy native mutex.
The first abort-wake test then exposed a lazy waiter mutex allocation during
refund. Both failed logs remain alongside the corrected runs. Neither
zero-allocation assertion was weakened. Native sanitizer runs use the installed
1.95.0 compiler with development-only AddressSanitizer flags; production features,
toolchain selection and runtime configuration are unchanged.

## Remaining boundary

The permanent root Arc, native mutex storage, node allocations, tracking vectors,
iterator/removal buffers, nested key/value payloads, initial State construction,
and global admission policy still need complete ownership and funding. The async
linear cell is outside Iroha's enabled dependency features and is unchanged.
Whole-operation admission must precede allocation and retain charges until real
reclamation, without acquiring partial credits while waiting for the remainder.
The open-ended map mutation API cannot silently become a completely prepaid
operation. Node cloning, tracking-vector growth, removal and lazy mutable
iteration must have explicit admitted bounds. Charge-bearing buffers and nodes
can be reclaimed during mutation or `pre_commit` while locks are still held;
their refund notifications need an allocation-free deferred owner released after
those locks. Deferring only the outer cursor charge does not close that boundary.
Complete live Validate/Apply handoff, retirement and participant durability,
recovery, and unchanged four/seven-validator fault qualification remain open.
No liveness goal or release gate is closed by this checkpoint.
