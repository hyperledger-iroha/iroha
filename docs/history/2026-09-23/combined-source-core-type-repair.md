# Combined-source Core type repair — 2026-09-23

Scope: the existing `optimizations` checkout only. A concurrent, still-unfinished
merge brought in transaction-membership append and reward/stake reserve tests
after the transaction-history owner had changed its prepared identity and State
restore error types. This record covers a source reconciliation, not a release
candidate or a completed merge.

The membership append now stores and compares the same charged `Identity`
(`Shared<IdentityState, AllocationCharge>`) as its prepared block and committed
root. The test helper clones that identity. This removes the older `Arc<()>`
assumption and keeps the pointer-identity check on the actual preparation
owner. The reward and stake reserve restore test helpers now return the
typed `StateRestoreError` produced by the current Kura seed decoder.

These repairs preserve the append protocol and reserve semantics. A combined
`cargo test --offline --locked -p iroha_core --lib
local_storage_recovery_emits_no_block_rejection` build completed and its one
selected test passed. The resulting test binary then passed all 10
`state::storage_transactions::block::membership_root::append::tests::` cases,
all seven `state::reward_reserves::tests::` cases, and all three
`state::stake_reserves::tests::` cases. The test build emitted 62 warnings.
Other concurrent edits occurred while that binary was being built, so these
are scoped feedback, not final-source qualification. The full workspace and
final-source runs remain open. The no-legacy-codec guard passed on the current
checkout, and the historical archive verifier accepted 64,736 records and
67,311 occurrences. No files were staged or committed by this slice.
