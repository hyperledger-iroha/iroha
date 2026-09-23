# Transaction hot-tip and snapshot admission gap

The September 23 transaction-history preparation slice funds the prior-tip
batch, next identity, and Concread cursor before constructing them. It does not
fund the retained latest-block set or snapshot scratch. No independent
preallocation patch is sound for either path in the current ownership shape.

## Retained hot tip

`TransactionsStorage` retains `ArcSwapOption<BlockInfo>`;
`BlockInfo.transactions` is a `std::collections::HashSet<Key>`
(`crates/iroha_core/src/state/storage_transactions.rs:52-69`). The ordinary
carrier hashes are first collected into a `Vec` in
`crates/iroha_core/src/tx.rs:123-140`. The State block then collects a new
`HashSet`, extends it with merge entrypoints, and passes it to
`TransactionsBlock::insert_block`
(`crates/iroha_core/src/state.rs:53644-53670`). That method allocates the
retained `Arc<BlockInfo>` after accepting the already allocated set
(`storage_transactions.rs:604-621`). `Arc` clones, exact tip identity, and
retirement are used throughout its publication and reader paths
(`storage_transactions.rs:652-681, 744-766, 860-896`).

The original configured membership pool is already threaded through Kura's
`transaction_history_bytes` (`crates/iroha_config/src/parameters/user/kura.rs:31-32`,
`crates/iroha_config/src/parameters/defaults.rs:812`,
`crates/iroha_core/src/kura.rs:2612-2622, 3046-3048`). Its
`AllocationBudget::try_reserve_layouts` can reserve concrete layouts atomically
(`crates/mv/src/allocation.rs:220-266`), and `ChargedBuffer::try_from_charge`
keeps an exact backing-layout charge with its allocation
(`crates/mv/src/allocation/buffer.rs:144-170`). Neither constructor owns the
current `HashSet` buckets or `Arc` control allocation. Charging a predicted
`HashSet` capacity after collection would not fund work before it happens or
track the actual table layout and deallocation. Charging only a replacement
buffer would still leave the `Arc` shell and old set uncharged.

The smallest coherent hot-tip cutover must make the tip's immutable backing
and shared shell concrete charged allocations under the original membership
pool, construct both before publication or execution effects, and carry their
charges through readers, rollback, replacement, and retirement. It must also
move the existing `stage_canonical_carrier_membership` call chain to a typed
local refusal: ordinary execution currently maps that staging error to
`BlockValidationError::ExecutionContextInvalid`
(`crates/iroha_core/src/block/post_execution_tail.rs:52-67`,
`crates/iroha_core/src/block.rs:8479-8481`). A local capacity refusal must not
become a deterministic invalid-block verdict. A focused regression should hold
an old view, exhaust the exact pool by one byte, verify the committed tip and
staged predecessor are unchanged, then release that view and retry with the
same payload; replacement and repeated-commit semantics must still agree.

## Snapshot scratch

The committed and staged transaction serializers copy historical B+tree rows
into fresh `BTreeMap`s; staged serialization also merges the preceding tip
(`storage_transactions.rs:1368-1422`). Serializing each `HashSet` allocates a
sorted `Vec<&T>` in Norito (`crates/norito/src/lib.rs:8413-8432`). The
transaction JSON methods have infallible `String` signatures
(`storage_transactions.rs:1424-1442`). Complete snapshot capture builds an
unbounded `String` before the writer's resource checks
(`crates/iroha_core/src/snapshot.rs:133-164, 4331-4359`). Restore parses a
full JSON value and constructs `Arc<BlockInfo>` before historical pool
admission (`storage_transactions.rs:1448-1507`), then allocates a canonical
comparison `String` (`crates/iroha_core/src/state/deserialize_core.rs:82-105`).
`SnapshotJsonBudgetScanner` counts encoded bytes and estimated transient
items (`snapshot.rs:1777-1810, 2044-2065`); those estimates are useful input
limits, but they are not charges for the actual map, set, or output layouts.

Snapshot scratch belongs to the snapshot capture/restore owner, not the
transaction-history pool. A funded implementation needs a fallible Norito
output/comparison sink and a snapshot-owner reservation that survives through
the exact capture or restore attempt. It must thread local capacity and
allocator refusals through `SnapshotCaptureError`/restore without converting
them into malformed-snapshot or block-validation errors. A focused test should
force a capacity refusal before the first scratch/output allocation, prove no
snapshot is published and no State generation changes, release the original
owner, and retry the same capture/restore input.

The committed-view `BTreeMap` copy could be removed by streaming its already
ordered B+tree reader, but that only eliminates one scratch allocation. It is
not complete hot-tip or snapshot admission and would not establish the required
refusal and retry contract. No runtime code or limits were changed in this gap
record; no Cargo command was run while the shared build slot was occupied.
