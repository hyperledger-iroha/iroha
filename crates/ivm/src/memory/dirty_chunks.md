# Dirty-leaf allocation ownership

`Memory` owns a pending-commit bitmap and a since-baseline bitmap. Each backing
contains `ceil(leaf_count / 64)` initialized `u64` words. `insert` validates the
index before mutation, duplicate inserts do not change the count, and iteration
visits each set bit once in ascending leaf order. Unused final-word bits are
always zero. Clearing or copying same-geometry words cannot allocate.

The funded constructor admits the image, fixed leaf array, and both complete
bitmap layouts before allocating them. Each bitmap consumes original
`ExecutionMemoryLease` credit into its `ExecutionBuffer`; no replacement budget
or unchecked capacity growth exists. A local image instead uses the existing
`OwnedAllocation` cache owner. Both variants retain and activate their original
allocation charges, and the final backing drop releases the charge. The image's
scratch estimate excludes bitmap bytes because their owners charge them once.
Snapshot plans use full bitmap geometry regardless of the current set-bit count.

The incremental memory commit passes the original bitmap directly to
`ByteMerkleTree`. It locks the canonical cache before leaves, matching the root
rebuild lock order. Sparse work hashes set indices directly; larger bitmap
updates split fixed leaf backing into disjoint 64-leaf groups and borrow the
corresponding original words. The canonical tree then receives exact ascending
leaf updates. The pending bitmap clears after successful root computation;
the since-baseline bitmap survives commits for warm reset. Large commits retain
the existing full-rebuild and hardware-selection paths.

A warm reset checks geometry before modifying memory, restores only the
since-baseline leaves, copies the template's pending bitmap into existing
backing, and clears the since-baseline bitmap. There is no warm dirty-tracking
reservation or allocation-refusal hook. IVM still refuses unrelated private
range copies before mutation and preserves program, lineage, and geometry checks.

TODO: Complete original execution funding for write logs, retained snapshot
scratch, canonical cached Merkle nodes, hardware staging and output buffers,
and remaining dynamic execution owners. This bitmap change does not establish
a complete execution-memory bound or make every memory-commit path allocation-free.
