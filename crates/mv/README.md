Library provides single value and key-value map structures.

For the execution flow specific for the blockchains:
1. transactions are grouped in blocks and executed one by one (every transaction is atomic)
2. blocks are committed sequentially (every block is atomic as well so either effect of every successful transaction is visible or no effect)

Features:
- single writer/multiple readers
- transactional properties of transactions and blocks (rollback changes on drop or explicitly commit)
- ability to revert changes created in the latest block

The storage layer uses concread's B-tree maps and epoch cells. Its dependency
enables `maps`, `ebr`, and the existing `foldhash` backend explicitly; unused
async and adaptive-cache defaults are disabled.

Block and transaction overlays expose borrowed preimages and touched entries for
State projection. Storage entries are ordered by key; cell and map records keep
exact before/after values without making another value copy or change list.
Applied children retain the first block preimage, while dropped children leave
no block delta. A replacement block starts after reverting the discarded tip.

These APIs report touches, including no-op mutation and absent-to-absent removal.
Consumers must compare their canonical value projections before committing a
semantic delta. The storage layer does not choose a serialization, hash, read
witness, or complete State commitment; a delta alone does not bind untouched data.

`Block::try_detach` consumes the actual Cell or Storage block after a caller
admission callback. The detached owner retains that callback's resource guard,
the original ordinary/replacement mode, the published current/undo identity,
and exact candidate touches. Capture moves keys and preimages and copies only
touched final values. It releases every original writer; no EBR reader or
borrowed B-tree snapshot is retained. Untouched reads remain tied to the original
owner/version even when the detached delta is empty.

Identity is private, local and non-serializable. Every actual MV publication,
including undo-only commits, direct insertion and current-only replacement,
rotates the opaque token while the current/undo publication is excluded from
identity observation. Restoring JSON or projecting history creates a new owner
while preserving the exact serialized values and undo. Equal values after a
later publication cannot recreate an earlier identity.

`Detached::try_prepare_publication` admits installation before reacquiring either
writer: acquiring an EBR writer itself clones its value. It acquires both
original writers without waiting, checks the exact captured identity under
those writers, stages current and undo, and preallocates the next identity.
A refusal returns the unchanged journal. A prepared publication can be aborted
back to that journal without changing visible state. Publishing consumes the
prepared pair and returns both capture and installation reservations to the
aggregate caller; the caller owns their eventual transfer/release.

These are component operations, not cross-field finality or atomic visibility.
The aggregate owner must prepare every component and retain all writers before
publishing any component. Admission covers current/undo copies, staging overlap,
Concread publication allocations and retained-reader reclamation; publication is
not promised to be allocation-free. Data writers precede the identity lock;
identity observation never acquires a data writer while holding that lock.

TODO: compose these component publications with exact aggregate State predecessor
ownership, membership, hash history, archive/resource reservations and finality.
The production State publisher and its resource policy remain unfinished; no
State execution, native-output or publication guard is bypassed by these APIs.
