# Charged cursor and retirement buffers

Work remains exclusively in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
The previous checkpoint is `dist/sumeragi-main-work/validation115.json`; this
implementation's source/build join is `dist/sumeragi-main-work/validation119.json`.
Neither checkpoint establishes production retained validation or network liveness.

## One insertion engine and original ownership

The original B+tree cursor now carries its mode's concrete node charge and
move-only funding provider. Reads and iterators preserve that charge type in
all node pointers. Original writer input supplies the provider and both tracking
buffers. `pre_commit` takes the original retirement buffer into the previous
reader's write-once owner; it creates no replacement buffer and releases no
retirement credit early. Detach/reattach retain the same cursor and buffers.

The sealed bookkeeping interface has two storage owners: the ordinary untracked
Vec and an exact fixed `Box<[MaybeUninit<NodePtr>]>` with its original charge.
The fixed owner exposes checked array layout before allocation, initializes only
its active prefix, cannot grow or clone, and frees the actual backing allocation
before invoking its charge destructor. Empty/zero-sized storage consumes no
allocation. Invalid layout returns the same charge before allocation.

A fixed-buffer insertion checks complete structural capacity before mutation.
For a path with h branch ancestors, at most h+1 original nodes are retired;
at most 2h+3 new pointers cover path clones, all split siblings and a new root.
Checked arithmetic and the balanced tree's addressable depth bound keep this
preflight finite. Insufficient room returns the original moved key/value without
allocation, mutation, or additional credit acquisition. This closes the reproduced
raw-node leak that occurred when a fixed buffer rejected registration after node
allocation. The disabled-preflight counterexample and restored-source digest are
retained in `generation119/tracking-overflow-counterexample.json`.

## Shared-reader safety

Read traversal and original-node clone probes now construct shared references.
Mutable references are acquired only for newly cloned nodes or nodes already
private to the writer's transaction. Overlapping forward/reverse readers retain
valid old items across mutable access/removal and later publication. Root and
cursor thread traits require shared key/value/charge safety; a writer also
requires its actual funding provider's thread safety. Compile-negative controls
cover non-Send/non-Sync charges, payloads and providers without imposing a
provider requirement on readers that do not own it.

## Evidence and remaining work

Actual System allocation/deallocation observation covers exact layouts, original
buffer and charge moves, zero-allocation handoffs, fixed-capacity refusal and
same-entry retry, abort, clone unwind, source-map destruction with detached work,
and original retirement buffers across successive commits and independent reader
release. Focused tree tests include forward/reverse traversal and ordinary
split/removal behavior. The final receipt records vendor feature/sanitizer runs,
workspace consumers, Core regressions and formal/source checks separately.

The public map and State still select explicit Untracked custody. Fixed buffers
are private primitives with narrow non-test dead-code expectations until the
closed admitted map API is connected. Full node/payload demand must be reserved
before cursor construction; this structural preflight is not that budget policy.
Ordinary K/V Clone, iterator stacks, initial control storage, MV undo and complete
State accounting remain unadmitted. Callback-bearing charges require the original
pool's notification-deferral scope around construction, publication and cleanup.
Complete retained Validate/Apply and retirement integration plus unchanged real
four/seven-validator fault/restart/final-transaction campaigns remain open.
