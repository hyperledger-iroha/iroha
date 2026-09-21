# Joint admitted Storage removal

The prepaid MV transaction now uses one shared edit planner for insertion and
removal. It reserves current-tree changes, the first block preimage, transaction
touch storage and nested copies together before changing any owner. Original
component reservations partition that single admission; they cannot acquire a
second pool. Removal borrows its query key and returns the removed private value
with its original allocation charge.

A missing current key still needs an explicit absent-to-absent touch and a first
undo `None`. It can therefore require admission even though raw map removal would
do no work. Repeated missing removal needs no additional storage once both
witnesses exist. It does not make a clean block dirty. An applied sibling cannot
replace an earlier first absence with its own inserted value.

The existing current and undo checkpoints remain the only rollback owners. Failed
admission leaves them unchanged. The transaction keeps its failure flag armed
through copies, mutation, unused-credit cleanup and provider destruction. A caught
panic cannot apply partial work. Abort restores the original parent roots and
buffers without allocating, including when the finite pool is full. Old readers
and values returned to the caller retain their own original charges.

Checkpoint132 passes the complete MV suite in both default and skinny tree
layouts: each runs 231 runtime tests and one documentation test without warnings.
The seven new controls observe real allocations and refunds, whole-demand and
planning refusal, missing-key witnesses, retained return values, full-pool abort
and clone/cleanup panic. An independent deterministic trace compares both storage
modes with a separate map model across block and transaction preimages, applied
and aborted siblings, tree rebalancing and retained readers. Production Core
compilation also passes. The receipt `dist/sumeragi-main-work/validation132.json`
records exact test artifacts and source captures, current structural/hygiene gates
and the explicitly unchanged scopes of earlier formal checks. This is focused MV
and Core qualification, not a fresh full-workspace or network run.

The first run preserved one incorrect failure-test expectation: a panic inside
the tree mutation deliberately keeps the original cursor failed after rollback.
The corrected test requires parent read/publication refusal and unchanged committed
state, while retaining exact credit and reclamation checks. A panic in the external
preimage copier, before either tree edit, still restores a usable parent. The
production failure guard was not relaxed.

This completes the closed removal operation in prepaid Storage; it does not
activate funded World execution. Native mutex/runtime and identity storage,
concrete model policies, general closed mutation/replacement, State generation
refusal, aggregate memory/work admission and the complete retained production
Validate-to-Apply path remain open. L1–L6 and unchanged real four/seven-validator
network qualification remain open.
