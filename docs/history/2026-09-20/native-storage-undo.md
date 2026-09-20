# Native Storage undo ownership

Checkpoint125 replaces production MV Storage's block-undo
`EbrCell<std::BTreeMap<K, Option<V>>>` with the same B+tree engine used by its
current state. It advances the active liveness goal without enabling funded
production mutation or closing L1–L6. Work remains in the authorized
`/Users/takemiyamakoto/dev/iroha` checkout on `optimizations`; the external merge,
index and unrelated changes are preserved.

## One engine for the original current and undo pair

Ordinary block opening now clears the private undo tree without cloning every
previous undo value. Original reader generations retain the old nodes and their
payloads. Block, detached and prepared-publication owners carry exact B+tree
cursors for both current and undo. Reattachment checks each original base;
changed raw undo generations refuse even if their logical contents return to an
equal value. Partial acquisition still releases the acquired undo writer and
returns the original pair for retry. Publication retains the existing pair
identity, inverse writer-commit order and undo-only commit behavior.

First touches remain explicit, including no-op borrows and absent-to-absent
removals. Native undo insertion checks membership independently of an entry's
`None` preimage, so an absent value cannot be overwritten by a later touch.
Applied child transactions preserve the first block preimage; aborted children
retain their existing rollback and dirty-before semantics.

Replacement blocks borrow committed undo entries and copy the required
preimages into the new current generation before clearing their private undo
writer. They cannot move payloads out of shared nodes. Snapshot, history,
projection and JSON consumers now read the native tree directly; explicit
restore inputs collect owned data only at their construction boundary. The JSON
shape and canonical entry ordering are unchanged. No fallback undo engine or
cloned compatibility read image was introduced.

Detached owners expose immutable iteration tied to the retained cursor. The
full-tree iterator also fixes its stale size hint: remaining length decreases
from either end, including alternating iteration and repeated exhaustion. This
preserves the exact-size touched-entry API. Range iterators keep their separate
conservative upper bound.

## Evidence and limits

The focused MV run passes 188 tests: 139 library, 18 admitted-map, 13 linear,
5 EBR and 13 map-generation tests. The same 188 pass under native AddressSanitizer
with ordinary and skinny node geometry. Strict MV library/test Clippy passes.
New controls prove clone-free ordinary block opening while an old undo reader
retains its values, raw undo-generation/ABA rejection, exact undo payload pointer
retention across writer contention, and mixed-end iterator lengths. Existing
snapshot, history, JSON, no-op, first-preimage, detached publication and rollback
assertions remain intact. The original release-notification panic control now
triggers a real replacement preimage clone under both writers, preserving its
registered-waiter, wake-after-unlock and poison assertions.

The actual vendor default, skinny and AddressSanitizer suites pass 298, 296 and
298 tests respectively. All 63 vendor/harness inputs match before/after/current
captures. The first MV development check exposed the missing owned iterator
accessor; the canonical borrowed accessor fixes that boundary without allocating
a copied entry list. Its failed log is retained.

`dist/sumeragi-main-work/generation125/` holds source captures, build and test
logs. `validation125.json` records terminal Core build/runtime and canonical-gate
outcomes with their exact source and executable scope; a preceding passing
artifact is not substituted for a current failed or incomplete check.
Checkpoint124 and earlier receipts retain their original scopes.

Both production trees still use explicit Untracked mode. The transaction-local
standard map and inverse-edit Drop remain unfinished. Complete joint admission
must fund current edits, first preimages and transaction bookkeeping before
mutation, then use already funded checkpoints for abort. Closed removal/update,
concrete model payload policies, iterator/native-runtime storage, aggregate State
budget configuration and complete Validate-to-Apply/retirement cutover remain
required. No full-workspace, release or real four/seven-validator qualification
is claimed by these engine and owner checks.
