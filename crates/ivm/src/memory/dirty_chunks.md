# Dirty-leaf allocation ownership

`Memory` owns a pending-leaf bitmap and a since-baseline bitmap. Each backing
contains `ceil(leaf_count / 64)` initialized `u64` words. Until the first root
or path request allocates the Merkle storage nothing commits, so the pending
bitmap holds every leaf written since the zero image. `insert` validates the
index before mutation, duplicate inserts do not change the count, and iteration
visits each set bit once in ascending leaf order. Unused final-word bits are
always zero. Clearing or copying same-geometry words cannot allocate.

The funded constructor admits the image and both bitmap layouts. Each bitmap
consumes its original `ExecutionMemoryLease` into an `ExecutionBuffer`. The
first root or path request admits the complete Merkle storage from the image's
original execution pool. A refused request returns that credit and preserves the
pending leaves for a retry. A warm runtime retains its allocated tree across a
same-geometry reset. Dropping its final backing releases the tree's charge.
Local images use the `OwnedAllocation` cache owner. The scratch estimate excludes
bitmap bytes because the two backings charge them once. Snapshot plans use full
bitmap geometry regardless of how many bits are set.

The incremental memory commit passes the original bitmap directly to
`ByteMerkleTree`. It locks the canonical cache before leaves, matching the root
rebuild lock order. Sparse work hashes set indices directly; larger bitmap
updates split fixed leaf backing into disjoint 64-leaf groups and borrow the
corresponding original words. The canonical tree then receives exact ascending
leaf updates. The pending bitmap clears after successful root computation;
the since-baseline bitmap survives commits for warm reset. `union_with` merges
a same-geometry bitmap in place and recounts, without allocating. Large commits retain
the existing full-rebuild and hardware-selection paths.

A warm reset checks geometry before modifying memory, restores only the
since-baseline leaves, rewrites the pending bitmap in existing backing, and
clears the since-baseline bitmap. The pending bitmap becomes the template's when
both images have, or both lack, allocated Merkle storage; otherwise the worker
keeps its own pending leaves and adds the restored ones. There is no warm dirty-tracking
reservation or allocation-refusal hook. IVM still refuses unrelated private
range copies before mutation and preserves program, lineage, and geometry checks.

TODO: Complete original execution funding for write logs, retained snapshot
scratch, canonical cached Merkle nodes, hardware staging and output buffers,
and remaining dynamic execution owners. This bitmap change does not establish
a complete execution-memory bound or make every memory-commit path allocation-free.
