# Charged transaction hot-tip cutover — 2026-09-23

Scope: the existing `optimizations` checkout at
`/Users/takemiyamakoto/devstuff/iroha`. This is a source-level F02 slice, not
candidate qualification. No branch, worktree, staging, or commit was created
by this work.

`TransactionsStorage` previously retained an uncharged `Arc<BlockInfo>` and
`HashSet<Key>` after the historical membership pool had admitted only B+tree
nodes, identities and prior-tip preparation. The committed tip now uses an
exact-layout charged `concread::shared::Shared` shell and a sorted fixed
`mv::allocation::ChargedBuffer<Key>`. A short `RwLock` protects only pointer
replacement and cloning; reader-held strong references keep the original
shell and backing charges until their actual retirement. The storage sequence
still checks the tip and history as one cut, and the replacement/rollback path
retains its previous tip.

Production `StateBlock::stage_canonical_carrier_membership` no longer builds a
second `HashSet`. It passes its existing ordinary carrier `Vec` to
`TransactionsBlock::try_stage_block`, which atomically reserves the concrete
shell and maximum key-array layouts before filling either. It checks
ordinary/merge overlap, sorts and deduplicates in place, and keeps the complete
backing charge even when duplicate entries shorten the logical set. Pool and
allocator refusal leave the staged tip unchanged and retain a typed
`MembershipAdmission` source for the local block-validation classification.
If a tip is already staged, the caller-owned Vec is sorted and deduplicated for
an exact set comparison with that tip; neither a second shell nor another
history-pool charge is needed for an idempotent or changed-payload decision.
The canonical JSON field names and sorted key-array bytes are unchanged.

The new focused tests are
`charged_tip_matches_canonical_hashset_json_after_duplicate_compaction`,
`exact_capacity_repeat_checks_original_tip_without_second_charge`,
`one_byte_tip_refusal_keeps_old_view_and_retries_same_source`, and
`ordinary_merge_overlap_refuses_without_staging_a_tip`. They cover exact JSON
parity with the former HashSet rendering, a fully occupied pool on repeat,
one-byte-under refusal with the old view pinned and the same borrowed source
retried after release, snapshot JSON restore, and overlap rejection before tip
installation. Existing replacement, repeated-commit, detached-publication and
serialization suites remain necessary. `rustfmt --check --edition 2024
--config skip_children=true` and `git diff --check` pass for the touched Rust
files. A fresh combined Core test binary compiled, and the focused
`storage_transactions::tests::` selector passed 22/22, including all four new
hot-tip tests. The broader transaction-history, Core and release matrices
remain pending.

The test-only retirement marker adds a field to `BlockInfo` under `cfg(test)`.
The one-byte refusal is exact relative to that test build's `Tip::layout()`;
it is not an absolute production memory-ceiling measurement. A release
candidate must measure the shipping layout and complete full-size resource
qualification separately.

The next F02 cutover must use this same original transaction-history budget
before execution effects: admit a fixed source-key backing inside the ordinary
`state_block_for_execution` pristine callback, before NPOS or QueuePlan
staging, and retain it in the returned `StateBlock`. Certified-merge staging
needs the authenticated sidecar's source count before its execution callback;
Native staging needs the verified group count in
`with_native_lane_execution_scope` before its start hooks. Replace the
incremental `merge_carrier_entrypoints` HashSet, its expected-set rebuilds and
Native seal copy with one funded canonical source owner. The ordinary Vec is
currently built after execution in `tx.rs`; moving only its final tip charge
earlier would not fund that allocation. A pre-effect one-byte refusal, same
owner retry, alias/duplicate parity and replacement/restart tests are required.
The Native retained candidate can carry its original State owner; ordinary
`validate_candidate` and `validate_and_apply` still re-execute separate
StateBlocks, so their full original-owner retry requires the shared Apply
retention cutover. This is a planned seam, not an implemented gate.

F02 remains **OPEN**. `canonical_carrier_membership_hashes` in `tx.rs` allocates
its ordinary `Vec` before this tip charge, and State's
`merge_carrier_entrypoints` is an uncharged `HashSet`. The production staging
method consumes the Vec, so a refusal leaves the authoritative signed carrier
available but does not retain that exact temporary Vec in the aggregate Apply
owner. The caller must still fund those source allocations before execution
effects and retain the original attempt across local retry. Snapshot JSON parse,
comparison and output scratch remain outside this pool's charged ownership,
as recorded in the preceding
[`transaction-hot-tip-snapshot-admission-gap.md`](transaction-hot-tip-snapshot-admission-gap.md).
