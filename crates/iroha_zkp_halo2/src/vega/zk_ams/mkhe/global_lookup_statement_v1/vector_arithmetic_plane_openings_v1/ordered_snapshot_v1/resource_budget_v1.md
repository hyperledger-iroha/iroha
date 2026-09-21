# Ordered stored-opening resource custody

This contract belongs to `ordered_snapshot_v1.rs` and its
`resource_budget_v1.rs` child. It covers the actual two-file storage owner,
not production source/proof authorization.

Every constructor requires a mutable caller-retained
`OrderedStorageSessionBudgetV1`. The admission issuer is not cloneable and has
no reset API. Move-only pair reservations retain its shared ledger after the
issuer is dropped. Independent writers and snapshots admitted by one issuer
therefore compete for the same counters.

Admission validates the complete plan, then atomically reserves both logical
file lengths and twice that total for ciphertext writes plus the complete
seal authentication pass. This occurs before either leaf creates a file,
samples a key or sizes storage. The immutable profile ceilings remain
16 GiB of live spool and 64 GiB of authenticated I/O. For the canonical pair,
reservation is 5,026,665,600 live file bytes and 10,053,331,200 write/seal bytes.
Test-only limits may be lowered; production cannot configure larger limits.

A write consumes its full 16,400-byte record reservation before the actual
crypto leaf call. Each seal consumes the entire leaf's file length before its
complete authentication/hash pass. A sealed pair must have consumed every
reserved write/seal byte. Subsequent authenticated reads consume one complete
record each, including repeated reads; they cannot spend bytes reserved for
another live writer. Context, geometry, slot and semantic write checks run
before the relevant I/O debit. Read semantic validation necessarily follows
authentication, so an authenticated but invalid plaintext retains its charge.

Local admission pressure returns a distinct `Capacity` error before I/O. Read
capacity refusal restores the exact existing pair, identity and reservations;
it is not invalid-proof evidence. A retry can succeed when another writer
releases unspent reservations. Corruption, ordering, authentication and semantic
failures still consume the pair. Failed or partial I/O retains its full authorized
charge; no returned error refunds attempted bytes. The leaf's fixed-offset
read/write loops advance only through the remaining requested slice, so
partial progress does not exceed that conservative record charge. File
metadata calls, kernel-internal work and retries below the file API are not
byte-counted as authenticated record traffic.

The original crypto leaf unlinks its empty file and verifies detachment before
key generation and sizing. The pair exposes no descriptor, pathname or reopen
operation. Both leaf owners close before the associated reservation releases
live-file credits; only unattempted write/seal reservations are refunded.
Creation failure, second-leaf failure, cancellation, poisoned storage,
snapshot drop and unwind preserve this lifetime. Mutex poison refuses all
new admission/I/O; destruction may release only already-owned credits.

The counters cover live logical detached-file lengths and conservatively
admitted authenticated record bytes. They do not measure resident memory,
allocator or Arc/Mutex control overhead, filesystem metadata, page cache,
physical secure deletion or forked descriptors. The existing 512 MiB resident
cap is unchanged and remains independently unqualified.

The tests use real encrypted files with a two-plane geometry for lifecycle,
concurrent reservation, temporary capacity refusal with retained identity and
retry after another writer drops, repeated reads, cancellation and
second-leaf failure. Their values are storage fixtures, not authenticated
production witness sources or proof evidence. Canonical plan arithmetic and
full limits are checked without allocating full-size files. Neither the
5 GB deployment path nor whole-proof resource qualification is established.

TODO: retain one such issuer in the eventual production proof-session owner
and pass it through every source and qPCS spool producer/reader. The current
source-to-ordered-writer context schedule, commitment-bound complete-pair
replay, Q-mask producer and proof/materializer seals remain unresolved. A
caller creating independent issuers does not enforce a whole-proof cap; the
current pair integration must not be described as that final admission path.
