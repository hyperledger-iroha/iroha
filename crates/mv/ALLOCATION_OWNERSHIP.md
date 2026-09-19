# Concread allocation ownership

MV uses the workspace-pinned `concread 0.5.10` source in `vendor/concread`,
with explicit features `ebr`, `maps`, and `foldhash` and defaults disabled.
The original crates.io archive checksum is
`6588e9e68e11207fb9a5aabd88765187969e6bcba98763c40bcad87b2a73e9f5`;
`vendor/concread/IROHA_PATCHES.md` records the local patch and retained license.
These notes identify actual allocation owners and the finite requested-layout
credit primitive. They do not establish a configured State-wide memory budget
or complete publication authority.

## Actual allocation and reclamation paths

| Owner | Allocation | Actual release |
| --- | --- | --- |
| `EbrCell<T, Charge>` | `src/ebrcell/mod.rs`: `new_charged` allocates the initial `Atomic<Allocation<T, Charge>>`; admitted writer acquisition clones `T` into its own `Owned<Allocation<T, Charge>>`. Commit moves that exact allocation. | `commit` and cell drop use `defer_reclaim`; abandoned writers reclaim directly. The charge is extracted, the original payload and allocation are destroyed/deallocated, and only then is the charge released. Unrelated epoch pins delay collection. Clone/destructor unwind conservatively retains the charge. |
| Original EBR successors | `EbrCellWriteTxn::detach` moves the exact unpublished allocation into `EbrCellOwned`; reacquisition moves it back without cloning. MV retains both current and undo owners and a prebuilt next publication identity. | Abort returns the original owners; commit transfers each charge with its allocation. Target/predecessor authentication occurs under both MV writer locks. |
| B+tree nodes | `src/internals/bptree/node.rs`: `Node::new_leaf`, `new_leaf_ins`, `new_branch`, and both `req_clone` implementations allocate `Box<CachePadded<Leaf/Branch>>`. Leaf clones also clone keys and values; branch clones clone keys. | `Node::free` dispatches to `Leaf::free` or `Branch::free`, reconstructing and dropping the exact allocation. |
| B+tree ownership transfer | `src/internals/bptree/cursor.rs`: `CursorWrite` owns newly allocated nodes in `first_seen` and tracks replaced nodes in `last_seen`. `SuperBlock::pre_commit` moves `last_seen` into the previous `CursorRead` and clears `first_seen`. | Writer abort frees `first_seen`; `CursorRead::drop` frees its `last_seen`; `SuperBlock::drop` frees the remaining current tree. Clearing `first_seen` on commit transfers ownership and does not reclaim nodes. |
| Reader-generation chain | `src/internals/lincowcell/mod.rs`: initial writer acquisition preallocates its next reader Arc shell; commit initializes and links that same allocation. Detached writers retain their exact base reader and shared root, and adoption checks both identities under the writer lock. | `LinCowCellInner::drop` drains uniquely owned successor links. An old reader can retain intervening generations even when those generations have no direct readers. |

Neither writer release, successful publication, nor a count of MV views proves
that these allocations were reclaimed. In particular, the current MV detached
publication APIs return admission owners at commit; a caller must not release
memory charges for values that remain in published or retired generations.

## Required hook boundary

Charged MV Cells require a prepaid current/undo pair before either writer clone.
Both original allocations survive detachment, physical contention, abort and
publication; detached installation does not reconstruct payloads. Actual State
field instantiations still use the explicit untracked mode pending exhaustive
admission. Storage also retains its original B+tree cursor and undo allocation,
without an after-value vector or installation replay. Its tree keeps both the
original base reader and actual shared root alive; foreign or changed bases refuse
without replacing the owner. Remaining dependency work must attach charges to each
actual node through commit/abort to the real free boundary. Charge node layouts inside Concread, where the concrete cache-padded
allocation types are known; do not estimate them from MV entry counts. Account
for cursor vectors and reader-chain allocations at their own owners as well.

Nested `K`/`V` allocations need separately admitted clone-footprint accounting
or allocation owners. A node-layout charge alone does not cover nested values.
`Leaf::drop` and `Branch::drop` deliberately skip initialized contents when
`FLAG_INVALID` remains set after a panicking clone. An accounting hook must not
release nested charges for those leaked contents as though they were freed.

Focused controls must cover aborted writers, retained readers across multiple
commits, tree splits/removals, map destruction, delayed EBR collection caused by
an unrelated epoch pin, and clone unwinding. Assertions belong at actual value
destruction/node free, not at view release. Existing Concread node-ID checks and
EBR drop-observer tests identify useful instrumentation points but are test-only.

The in-repository dependency patch starts at the EBR allocation boundary.
Exhaustive State integration, B+tree node and cursor custody, nested payload
accounting, and a configured aggregate resource policy remain required. Registry-cache
edits and a carrier-only reservation wrapper cannot supply this ownership.

The existing `NexusStorage::max_wsv_memory_bytes` policy bounds estimated hot-tier
weight, permits grace/unspillable overflow, and excludes these retained generations.
It cannot silently serve as the aggregate resident budget. That policy and exhaustive
typed payload allocation support remain required; encoded-length multipliers and
unit admission are not substitutes.

## Finite requested-layout credits

`AllocationBudget` has an immutable finite limit, including a meaningful zero.
Reserve the complete checked sum of real layouts before construction, then split
its move-only reservation into charges attached to the actual allocation owners.
An impossible demand refuses immediately; occupied capacity carries a release
observation from that exact pool. Actual deallocation precedes credit return and
notification. Current, retired and unpublished EBR generations share the pool.

Reserve each complete operation atomically. A capacity retry must not wait while
retaining partial credits whose own release is necessary to admit its remainder.
This primitive accounts requested layout bytes, not RSS or inferred nested
payloads. Cursor/node storage, nested values, identity/collector/control storage
and file descriptors need their own complete ownership and admission paths.

## Acquisition unwind

A raw writer can panic while cloning a generation before returning its guard.
Cell and Storage wrap that acquisition as well as the returned physical guard:
existing retries wake only after the raw lock has unwound, then observe typed
poison. Normal successful acquisition emits no premature release signal.
Deterministic regressions register the busy waiter before triggering the clone
panic. Writer release is still distinct from allocation reclamation.

## Original map publication

Initial synchronous writer acquisition allocates the working cursor box and next
reader Arc shell. That same cursor allocation survives detachment, refusal,
reattachment and abort; publication consumes it without reconstruction. Keeping
the descriptor small also avoids copying the complete cursor into aggregate
retry errors. Unpublished work is destroyed before its base reader and shared
root; adoption checks both exact identities under the original writer lock.

Reader links and retired-node vectors are write-once owners. Their `OnceLock`
fields avoid first-use native mutex allocation during commit. The permanent
reader mutex is initialized during map construction and reused thereafter. Scalar
controls require zero allocation calls through detach/retry/abort/publication,
including the first commit without any prior reader. This does not claim that
arbitrary payload destructors or other linear-cell implementations cannot allocate.
