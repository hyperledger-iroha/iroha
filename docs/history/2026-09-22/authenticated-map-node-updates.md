# Bounded updates of externally owned canonical map nodes

The existing authenticated map now prepares insertions, value replacements and
removals through a caller-owned immutable node store. It authenticates the
original root and search path, compares the actual preimage, checks entry-count
arithmetic and recognizes exact no-ops before writing. It copies only the changed
path, splits divergent compressed prefixes and collapses unary branches after
deletion. Untouched subtrees retain their original content references.

The caller supplies one fixed-size workspace. Lookup and workspace use the same
257-node bound; an update reads at most 257 descriptors and writes at most 257
nodes, independent of retained history. There is no heap allocation or resident
history reconstruction in this kernel. Store allocation admission, physical
encoding, durability, immutable content ownership and provisional-node retirement
remain explicit caller obligations. A prepared root is returned only after all
required writes succeed. Failure or unwind may leave unreachable new nodes; it
does not publish a root or change any original binding under that store contract.

Six new controls compare external edits with canonical resident and cold rebuilt
maps, retain every old version, delete complete maps in both orders, inject failure
at every read/write boundary for insertion/replacement/removal, retry after partial
writes and unwind, reject invalid authority/preimages/counts/storage before writes,
and exercise every valid split bit in both insertion orders. A default-thread-stack
control reaches the full 257-read/write bound, including an in-memory zeroized Hash
whose final-bit split cannot arise from ordinary marked Hash construction. The
14 prior map controls remain selected.

The complete Crypto test target compiles without warnings; all 20 map controls
pass with zero failures or ignores on 7,554 unchanged captured Rust inputs.
Downstream Core library compilation passes with the same inputs and one existing
warning for two unchanged, unused State cache helpers. Commands, source manifests, actual emitted test
executables, runtime logs and the root source review are retained under
`dist/sumeragi-main-work/generation161-crypto/`. The earlier 19-control run is
preserved separately from the final 20-control candidate.

This extends the [authenticated lookup prerequisite](authenticated-map-node-lookup.md).
Its earlier full Crypto suite and the preceding production repair checkpoint are
separate evidence; they are not relabeled as full-suite runs on this candidate.
No new wire/disk format or compatibility path is introduced. Original State root
and publication custody, authenticated restore, funded durable node storage and
incremental complete-State checkpoints remain required before history eviction.
Production Validate-to-Apply integration and unchanged four/seven-validator
fault/restart/final-transaction qualification remain open; no L1–L6 outcome closes.
