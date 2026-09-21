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
| Linear cursor and reader-generation chain | `src/internals/lincowcell/mod.rs`: charged acquisition receives both original shell charges before allocating either shell or constructing the writer. Private strong-only control blocks expose their exact concrete layouts; commit initializes and links the original reserved reader shell. Detached writers retain their exact base reader and shared root, and adoption checks both identities under the writer lock. | The last strong reference moves the payload and original charge out, frees the actual control block, destroys the payload, then releases the charge. `LinCowCellInner::drop` drains uniquely owned successor links; an old reader retains intervening generations and their charges. Payload unwind conservatively retains the affected charge. |

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
without replacing the owner. The dependency now attaches original typed charges
to actual padded nodes, fixed bookkeeping buffers and reader/cursor shells. The
public `Prepaid<P>` map mode plans one closed insertion under its original writer
lock, using exact storage layouts and an explicit nested payload policy before
constructing its successor. A further closed operation authenticates the same
root/base and admits an edit against its actual private tree. It retains the
original cursor and publication shell, grows only exhausted tracking buffers
under the new reservation, and publishes no intermediate generation. Refusal
returns the original successor and input unchanged. Unrestricted mutation stays
unavailable. Unused prepaid remainder returns before each handoff while allocated
charges remain in their owners.

Closed map removal plans the original search path and the exact sibling that
rebalancing can use at each level. For `b` branch levels, the engine needs at most
`2b + 1` new-node slots and `3b + 2` retirement slots, including merge and root
demotion. Planning includes actual padded layouts, nested node copies and at
most two additional separator copies per level. Complete admission precedes
buffer growth or mutation. Missing keys need no admission or allocation. The
same removal engine serves both modes; no alternate tree or inverse-insert
rollback is introduced. `MapAdmissionError` names refusal for acquisition and
all closed map edits. Storage removal joins this plan with its original undo
and touched-key demand before acquiring any new credits.

Attached writers in either map mode lend exclusive transaction checkpoints. A
new private generation tag forces edits to copy parent nodes; nested guards
resolve in LIFO order. Prepaid checkpoints retain the parent's original tracking
allocations when they grow. Abort restores the exact root, length and generation,
then frees only child allocations without allocating or reserving rollback
credit. Prepaid mode also restores the original buffers; Untracked mode may keep
grown vector capacity. Untracked saved-buffer slots are zero-sized because that
mode never displaces an original fixed buffer. Saved roots retain their non-null
identity, allowing resolved checkpoints to reuse the pointer niche. Apply keeps
edits private until the original writer commits. Caught mutation or cleanup panic makes that cursor unusable, including
for reading, detaching and publishing. Borrowed current and saved-root values
cannot outlive or mutate their checkpoint. Prepaid mode exposes only closed
admitted insertion, removal and reset; generic checkpoints do not grant ordinary
mutation.

Production Storage now retains both current and block-undo maps in this same
B+tree engine. Block opening clears a private undo root without deep-cloning the
previous undo values. Snapshots retain their original reader generation; detached
publication retains both original cursors and authenticates both bases on retry.
Replacing the previous block copies its required preimages from the retained
undo reader before clearing the new undo writer. Snapshot/history/JSON consumers
borrow native entries directly instead of constructing a compatibility image.
Production transactions retain checkpoints of both trees and borrow transaction
preimages from the saved current root. Dropping a transaction restores both
original trees without cloning, allocating or replaying inverse edits. Apply
checks both cursors and destroys ordered touch keys while both rollback guards
remain armed, then transfers both private changes and the dirty flag before
either checkpoint retirement can invoke user cleanup. Caught
preimage-clone or owned query-key destruction panic cannot apply partial edits.
Direct block edits retain their own failure state through first-preimage cloning
and owned-query destruction, including work outside either tree cursor. Every
value read, new edit, capture and publication preflights that state and both
cursor states. A caught child undo-cursor panic therefore cannot publish a
healthy current tree before discovering the failed undo owner.
The explicit prepaid insertion/removal transaction retains these same two checkpoints.
Checked generation refusal occurs before admission or payload copying. Its ordered
local touches use a concrete `Box<[MaybeUninit<K>]>` plus the original exact array
charge. Planning extends the canonical pair demand with checked array growth and
`ClonePlanning::plan_key`; the one original provider prepares the key and optional
array before either map edit. Existing keys move without cloning. Repeated touches
add no demand and preserve the original owned key. Installation has no payload
callbacks and returns the emptied old array for explicit cleanup while aggregate
failure remains armed. Abandonment leaves the original touch storage unchanged.

Touch destruction removes each key from the initialized prefix before its Drop;
if one destructor unwinds, a stack cleanup guard drains the remaining prefix.
The real array deallocates before its concrete charge refunds. A second destructor
panic retains ordinary Rust fail-stop behavior. Ordered iteration borrows the
initialized slice without allocating. Transaction cleanup, including a caught
checkpoint-apply panic, poisons the original block before it can publish.
These component operations do not enable prepaid State transactions; concrete
model payload policies and configured aggregate admission remain required.

The same MV Storage family also exposes explicit prepaid construction and
insertion/removal blocks. One checked startup reservation is partitioned between both
map owners; `Arc::ptr_eq` authenticates the original pool, independently of equal
limits or available counts. Both writer shells are admitted together, then a
closed reset admits the actual empty undo root and retirement bookkeeping.
Insertion uses one complete current/first-preimage demand while retaining both
original locks. A higher-ranked callback prevents writer guards from escaping
the whole-block refund scope. Node charges are actual `AllocationCharge` owners;
copied nested payloads require an explicit `AdmittedStoragePolicy`. Removal
preserves a first None preimage even when the queried key is absent and marks
dirty only for a present value. Owned query cleanup stays inside pair failure
guards. Both maps and their shared identity publish before any retired allocation
cleanup or retry notification, including an untouched current generation.

Prepaid replacement uses `try_with_admitted_replacement` inside the same closed
refund scope. It acquires undo before current, funds both shells together, then
borrows held undo preimages and admits each restoration's edit and incoming copies.
Undo clear follows all restorations. Capacity or planning refusal discards both
private trees, including an already restored prefix. Replanning refusal remains
typed; policy identity and demand are checked against the original pool.
Subsequent edits retain preimages from the restored state. Callback failure leaves
the published pair unchanged, and no physical writer escapes the callback.

`try_from_snapshot_admitted` restores both exact images into a private destination
using the same pool and payload policy. Borrowed snapshot and history accessors
support both map modes without copying entries. Explicit absent preimages survive
restoration; current values never stand in for missing undo information. Refusal
keeps source owners reusable and discards all private destination allocations.
Callers still authenticate source schema and generation-fence snapshot acquisition.
These operations admit each edit, not aggregate restoration work.

Private apply moves both checkpoints' retired bookkeeping before destruction,
then releases it with the parent failure flag still armed. Final pair publication
prepares both owners before transfer, publishes both roots and pair identity,
unlocks all participants, then destroys retirement and issues notifications.
Original readers retain their nodes until physical reclamation.

Publication identities use the same charge-owning shared allocation as Concread
linear cells. Initial admission includes the owner and first version; writer
admission includes the successor version. Construction uses the exact declared
layouts. Captured identities retain original charges, and final identity refunds
follow physical writer and publication-lock release, including unwind.

World currently instantiates Untracked Storage. Native mutex/runtime and
release notification storage remain outside constructor admission. Real
model payload policies, mutable access, State generation-refusal
propagation and aggregate execution/restore work admission remain unfinished.
Sorted touched-key insertion shifts its suffix and still needs a bounded work policy. Borrowed map iterators and ranges retain bounded traversal
state inline and make no heap allocations. Final-tree
teardown walks original child pointers with a bounded stack and allocates nothing.

Extract node charges before destroying their cache-padded Box and refund only
after deallocation returns. Dropping an ordinary charge field happens too early.
Likewise, dropping an Arc's payload does not prove its control block was freed.
The node-ID registry records entry into Drop; qualifying credits requires an
allocator-level witness that the actual deallocation preceded the refund.

Nested `K`/`V` allocations need separately admitted clone-footprint accounting
or allocation owners. A node-layout charge alone does not cover nested values.
B+tree clones now keep a fully initialized prefix throughout construction.
Leaf keys remain ordinary locals until their value clone also succeeds; branch
separator clones advance the prefix only after installation. Clone unwind drops
every completed payload and the actual unpublished node, without an invalid-node
bypass. Separator replacement drops its original initialized key; split, merge
and redistribution prepare fallible separator clones before destructive movement.
Redistribution transfers existing keys with their child pointers instead of
reconstructing an uninitialized destination after moving it. A payload destructor
can still itself panic; complete allocation custody must not refund leaked nested
storage as though reclamation completed.

Focused controls must cover aborted writers, retained readers across multiple
commits, tree splits/removals, map destruction, delayed EBR collection caused by
an unrelated epoch pin, and clone unwinding. Assertions belong at actual value
destruction/node free, not at view release. Existing Concread node-ID checks and
EBR drop-observer tests identify useful instrumentation points but are test-only.

The in-repository dependency patch starts at the EBR allocation boundary.
Exhaustive State integration, actual model payload policies, initial/undo
ownership, and a configured aggregate resource policy remain required. Registry-cache
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

`with_deferred_refund_notifications` encloses a synchronous operation whose
physical guards are acquired and released inside the closure. Reclamation
returns credits immediately but coalesces this thread's notifications for the
original pool until the scope exits, including unwind. Borrowed stack records
provide allocation-free nesting; unrelated pools and other threads keep their
normal progress. Detached allocation owners may escape after unlocking, but
physical guards must not escape. The closed map insertion returns a detached
owner after unlocking; its MV caller encloses admission and execution in this
exact scope and must fund every participating nested payload pool.

Notification retains the original waiter cohort through callback unwind. If a
wake or consumed-waker destructor panics, the remaining registrations are still
notified outside the locks before the original panic propagates. A second panic
during unwind retains normal Rust fail-stop semantics. The
[refund notification record](../../docs/history/2026-09-20/refund-notification-custody.md)
retains the reproduced lost-wakeup counterexample and scoped validation.

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

Initial synchronous writer acquisition allocates the working cursor and next
reader control blocks. That same cursor allocation survives detachment, refusal,
reattachment and abort; publication consumes it without reconstruction. Keeping
the descriptor small also avoids copying the complete cursor into aggregate
retry errors. Unpublished work is destroyed before its base reader and shared
root; adoption checks both exact identities under the original writer lock.
Attached abort releases the writer lock before private work destruction, exactly
as detach followed by abort does. Its retained base protects all shared nodes.
Charge callbacks run after unlocking; a refunded pool may synchronously wake a
retry. Construction unwind releases the poisoned lock before refunding either
uninitialized shell. Publication retains the cursor charge until both writer
and reader locks have been released and the new reader is visible.

Reader links and retired-node vectors are write-once owners. Their `OnceLock`
fields avoid first-use native mutex allocation during commit. The permanent
reader and writer mutexes are initialized during map construction and reused
thereafter. Release notifications initialize their mutex at construction and each
pending registration's mutex before publication, so first notification needs no
lazy native-mutex allocation. Scalar
controls require zero allocation calls through detach/retry/abort/publication,
including the first commit without any prior reader. This does not claim that
arbitrary payload destructors or other linear-cell implementations cannot allocate.

The [charged shell and teardown record](../../docs/history/2026-09-20/linear-allocation-custody.md)
tracks allocator-observed layouts, frees and refunds, concurrent reader release,
reentrant wake tests, and remaining production admission boundaries.

## B+tree clone and separator lifetime correction

The [current correction](../../docs/history/2026-09-20/bptree-allocation-lifetimes.md)
removes the deliberate partial-clone leak and initialized-key overwrite, and keeps
separator movement valid across clone failures. New branches retain their Box
through fallible debug verification. Tests track distinct nested payload
allocations through every clone failure position, actual writer unwind, detached
abort, ordinary rebalancing and retained reader-generation reclamation. This repairs
real destruction paths needed by node charging. The subsequent typed-node
implementation attaches charges to actual leaf/branch allocations; nested-payload
credits and a funded production validator remain unfinished.


## Original writer input and concrete node charges

`LinCowCellCapable::WriterInput` makes the constructor consume the same move-only
input accepted under the original writer lock. `WriterAdmission` joins that input
to both shell charges before either shell is allocated. Detached work retains the
original cursor/input; reattachment never admits or constructs another cursor.
Unit-input convenience methods require that exact associated type. Non-unit
untracked callers supply an explicit input callback; no `Default` constructor or
charged fallback supplies missing admission. The asynchronous implementation
shares the explicit input contract but has no charged-shell interface.

Inputs may themselves contain charged allocations. Their constructor can unwind
before returning to the cell; its nested destruction therefore needs the original
budget's lexical notification scope around the entire synchronous operation.
Actual allocator tests cover the same input Box across refusal, detachment,
contention and retry, and notification after unlock on abort/constructor panic.

The single B+tree node implementation carries its concrete charge type through
all leaf/branch pointers and node states. Every constructor, clone and split
requests the actual `Layout<CachePadded<Leaf/Branch<K,V,C>>>` from its explicit
funding owner before allocation. The charge resides inside that original padded
allocation. A construction/reclamation owner retains it through initialized-prefix
cleanup and comparisons, extracts it before payload destruction, then drops it
only after the exact Box has deallocated. Payload destructor unwind conservatively
retains credits. `Untracked` is zero-sized and preserves ordinary cache geometry.

The original cursor now carries its mode's concrete node charge, funding provider
and tracking buffers through the same insertion engine. Fixed tracking buffers
retain exact original backing storage and charges; retirement moves the original
buffer into its previous reader until actual free. A bounded insertion refuses
insufficient slots before mutation and returns the original entry. Shared reads
and clone probes no longer create mutable references to published nodes.

Every payload copy in the node engine now consumes its funding provider's
explicit `NodeCloning` policy, including separator copies during splits and
rebalancing. Node funding alone cannot select ordinary Clone. Concrete funded
policies must split prepaid ownership before nested allocation and retain the
original charge in the returned key/value until actual free. Moving initialized
slots preserves their existing owners. The production Untracked policy delegates
to ordinary Clone; tests separately exercise concrete charged payload copies.

The existing public map, read, snapshot and detached-owner types now carry their
concrete mode. `Prepaid<P>` admits one insertion using a checked bound for cloned
nodes, possible siblings/root, original fixed buffers, both shells and all
possible nested payload copies, including child minima outside the insertion
path that a split can promote. Planning allocates nothing. Admission refusal
returns the original input; a post-mutation panic aborts the private cursor.
Completed insertion drops only unused prepaid remainder before detaching.
`try_insert_owned_admitted` authenticates and extends that same cursor under one
new complete reservation. Existing pointer bookkeeping moves into a prepaid
geometrically grown buffer only when needed; old backing storage is freed before
its charge returns. Capacity refusal keeps the original buffers and entries.
Abort destroys the entire private successor without obtaining more capacity.

`AllocationBudget::try_reserve_bytes` accepts this already checked layout sum
without fabricating one aggregate Layout. Actual allocations still split exact
layouts and retain their own charges through physical free. Initial node, root
and reader custody is explicit; native mutex/runtime ownership, real model
payload policies, remaining MV mutation paths and configured aggregate State
integration remain open.
Real callback-bearing charges need their original notification-deferral scope
around physical guards and destruction. The [closed insertion record](../../docs/history/2026-09-20/closed-admitted-insertion.md)
records the initial operation. The [retained edit record](../../docs/history/2026-09-20/retained-admitted-edits.md)
records its multi-edit and root-ownership extension with separate validation.

EBR Cell publication uses the original exclusive writer to transfer each allocation
without entering the epoch collector. Opaque unlinked allocations remain owned
through both physical writers and pair identity rotation; their retirement runs
after unlock and retains the original reader grace period. This removes collector
callbacks from the transfer interval, but does not admit collector bookkeeping.

## Scoped detached Storage

Prepaid capture moves the same admitted current/undo nodes, next publication
identity and callback result out of the synchronous block scope. Both physical
writers release before either native release signal runs. No payload clone or
new capture allocation occurs. Original nodes and publication identities keep
their own charges after the source Storage is destroyed.

Joint reattachment requires the original pool's borrowed `AllocationScope`.
That token is neither Send nor Sync and cannot leave the higher-ranked budget
callback. Prepared owners borrow it through publication, abort, drop and unwind;
a detached journal may leave after both physical writers have released. Same-pool
budget clones authenticate; another pool with equal limits does not. This closes
Storage's owned handoff, while concrete World/resource integration remains open.

Block opening, execution and prepared-pair abandonment share one original writer
owner, established immediately after both acquisitions. Reset, replacement and
admitted snapshot copying retain this owner through every fallible operation. It
destroys both physical owners before either native notification, including when
private payload destruction unwinds. Both actual writer poison states are read
after destruction and retained before callbacks start. A cleanup or callback
panic does not stand in for a physical mutex poison verdict. Pool refunds remain
inside the original enclosing scope. Child transactions borrow this pair's same
original checkpoints; block publication and capture consume it once. Partial
acquisition still reports the actual held writer. Whole-State cleanup and complete
aggregate physical preparation remain separate required boundaries.

The canonical release implementation lives in Concread. Native active-reader
mutex observations identify that physical blocker independently of the writer.
Published retirement retains the same notification state by reference count after
both locks release; its deferred signal allocates no new source. Notification
construction and pending-future registration remain outside native constructor
admission. Complete aggregate abort and callback ordering remain required.

Detached preparation now retains native prepared commits and the identity guard.
Successful publication transfers those owners without another lock acquisition
and returns original cleanup plus reservations. Prepaid published cleanup retains
the same borrowed scope as physical preparation. World keeps this cleanup in its
original prepared boxes, and State retains the resulting field vector and runtime
cleanup through its physical fences. See the [component boundary record](../../docs/history/2026-09-21/prepared-component-retirement.md);
whole-State abort and effect-lock preparation remain open.


Prepared abort returns the original detached current/undo journal together with
its original reader, writer and identity notifications and installation owner.
An aggregate must retain the returned cleanup until every participant and outer
fence is unlocked. Prepaid abort cleanup borrows the original allocation scope;
only the detached journal may leave that scope. Returning the journal alone by
selecting the first tuple member intentionally finishes cleanup at that local
boundary and is unsuitable while an enclosing aggregate still holds writers.

Preparation refusal returns original journal, error and cleanup separately. No
installation reservation is refunded during a normal failed return. Original
identity probes and acquired writer/reader release owners remain in cleanup until
the aggregate unlocks. The prepaid refusal wrapper borrows the original pool scope;
a compile-fail control prevents that cleanup from escaping, while the original
detached journal may leave. Cleanup uses fixed optional owner slots, not a heap
queue or fabricated owner for an unacquired Busy lock. Native stale-map rejection
and panic propagation remain separate boundaries requiring original ownership.
