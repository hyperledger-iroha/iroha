# Membership segment ownership

`Kura::reserve_membership_range` lends one range from the original locked Kura
root and single generation-1 file descriptor. `[kura.membership_storage]` has
finite `max_bytes` (default 256 MiB) and `memory_bytes` (default 64 MiB); neither
accepts zero or an environment override. Membership history does not borrow the
block-hash budget. The concrete segment-control Box is charged before allocation
and freed before its original charge. The fixed budget and notification source
constructors remain Kura-constructor allocation obligations. Append owners must
charge their own exact codec/workspace layouts to `membership_memory_budget()`.

Reservations use checked 184-byte record intervals. The State codec checks that
size against its actual Norito frame. Actual segment bytes join Kura's fixed
physical inventory and disk scans; unused reservations join the existing carrier
admission sum. Quota admission is not a filesystem space guarantee: positioned
writes may be short or fail, including ENOSPC. No missing bytes become logical
membership absence. Only the caller's authenticated record/root checks can
establish membership.

The original Kura root lock and descriptor-relative create/open checks bind the
single file. Every I/O checks the original descriptor and namespace entry;
substitutions and extra links fail. There is no pathname reopen. This boundary
currently supports Unix except ESP-IDF; other platforms return `Unsupported`
instead of selecting a different storage implementation.

One active append owns the writable interval. The caller installs its exact
pending frame before I/O. The range additionally retains a bounded native-write
request and original physical accounting through ambiguous post-write metadata
failure. Recovery checks only that request's possible prefix on the same file.
It closes the short total-usage operation on error so unrelated cache rescans do
not hold canonical ownership while waiting for a later retry. Recovery publishes
the retained physical delta and invalidates cached totals instead of adding it
twice to a rescan that already observed the bytes.

`sync_data` does not publish a root. After authenticating complete records and
sealing its root, the caller uses `complete(end)` to close writes and advance the
readable frontier. Completion requires the exact descriptor length, full-frame
alignment, the synchronized current write state, and no touched bytes above end.
It returns only the proved never-written tail; repeated same-end completion is
idempotent. Earlier closed ranges remain read leases of their original complete
extents. `take_cleanup` transfers the original range event and both Kura-fence release batches to cleanup kept
outside all enclosing physical writers. Without transfer, the event stays with
the range until it drops. An incomplete Drop revokes its append permit, records
abandonment, and retains pending/uncertain disk capacity. Its actual release wait
wakes retries to observe `Abandoned`, not fictitious available capacity.

Range reservation, writing, completion and uncertain-write recovery acquire the
original prune/canonical guards; call these before enclosing Kura publication
guards. Both guard notifications remain in the range across operation return, errors
and unwind, until the complete outer cleanup is retired. Reserve before State
writer acquisition: a refused reservation has no range to retain its local
Kura-fence releases. The enclosing State owner separately retains its range release
and memory-refund notifications until all State siblings release.

TODO: authenticate restart roots and incomplete ranges, integrate the original
range into the complete State publisher/checkpoint, and provide authenticated
reclamation before production cutover. Existing segment files return
`RecoveryRequired`; their length cannot reconstruct root authority. No rollover,
GC, indefinite-history guarantee, or general query tier is implemented here.
