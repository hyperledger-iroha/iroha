# Closed admitted map removal

Date: 2026-09-21. Scope: the original Concread B+tree and its public prepaid
writer/checkpoint API, with real MV allocation-budget controls. This is a
prerequisite for shared BlockHashes history, not its production cutover.

## Ownership and sizing

Ordinary and prepaid removal now share one recursive engine. Before touching
nodes, it verifies key membership and preflights both original tracking buffers.
For a path with `b` branch levels, at most `2b + 1` nodes can clone and `3b + 2`
retirement entries can be needed. Those bounds include possible sibling clones,
merges and demotion of an empty branch root. No root replacement allocation is
needed during demotion.

The closed planner borrows the exact path and the sibling used by the existing
rebalance algorithm. It includes actual padded node layouts, nested payload-copy
demand, possible tracking growth and at most two extra separator copies per
branch. Actual child minima matter: equal ordering does not imply equal nested
allocation demand. Descendant minimum lookup can revisit bounded paths; total
pointer traversal is quadratic in height in the worst case, while planned node
and payload-copy counts are linear in height at fixed fanout. It never scans the
whole tree.

The original provider admits the complete plan before buffer allocation or
mutation. Refusal preserves the private root and buffers. Missing-key removal
needs neither admission nor payload planning, even for an unsupported copy
policy. Each successful operation returns unused reservation while allocated
charges remain in their actual owners. The returned value moves its private
allocation rather than cloning it again.

Checkpoints retain their original root and bookkeeping through deletion, sibling
rebalance and root demotion. Abort restores that ownership without inverse
insertion or rollback credit. A caught clone/rekey panic leaves the cursor
unpublishable. `MapAdmissionError` replaces the insertion-specific name across
acquisition and closed map edits; there is no compatibility alias.

## Verification scope

Four engine tests exercise undersized and exact fixed-buffer bounds, absent-key
no-op, both edge deletion directions, merge/root demotion, retained old readers
and clone unwind. Two public planner tests exercise unsupported descendant-minimum
planning and height-bounded payload visits without allocation. Four public MV
credit tests additionally observe every actual allocation/free/refund while
removing ascending, descending and permuted interior keys, retaining old readers,
refusing a full pool, applying nested removal then aborting at full capacity, and
unwinding at distinct clone positions.

The current command logs and source-bound receipts are under
`dist/sumeragi-main-work/generation129-final` and
`dist/sumeragi-main-work/generation129-vendor-final`. Final pass counts and the
joined source scope belong in `dist/sumeragi-main-work/validation129.json` after
those checks finish. No release or network acceptance is claimed by this record.

## Remaining production work

The live BlockHashes owner still copies complete history at block/read acquisition
and direct commit. Replace those vectors with one height-indexed shared tree,
using this admitted removal for actual tail truncation. Carry immutable native
generations through indexed/iterable readers and stream snapshot serialization.
Keep the emergency Fast mapped journal explicitly read-only.

Construction, identity/control storage, Strict snapshot loading and all edits
must receive the original configured resident pool before allocating. Local
capacity refusal must release original writers and retain a reachable release
wait through State and the existing Validate/Apply owner; it must never become a
semantic invalid-block verdict. The existing hot-tier WSV estimate is not that
resident pool. Concrete World payload policies and joint Storage removal/undo/
touch admission remain separate prerequisites. L1–L6 and real four-/seven-validator
fault/restart/final-transaction qualification remain open.
