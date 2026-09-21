# Snapshot payload read-buffer custody

This slice funds the **actual contiguous snapshot payload backing allocation**
used by Strict restore and authenticated generation validation during publication
and garbage collection. It does not complete multilane resource admission.
Decoded State/MV generations, nested values, parser and Merkle scratch, sidecars,
writer output, directory inventories and allocation-pool control storage remain
separate obligations. Native ingress and scalar State allocation policy are not
changed.

## Production ownership

`defaults::snapshot::MAX_READ_BUFFER_BYTES` defaults to the existing 1 GiB
maximum payload. `snapshot.max_read_buffer_bytes` is an explicit nonzero config
field, validated between `snapshot.max_payload_bytes` and
`snapshot.resources.max_transient_bytes`. The actual config reuses the validated
user representation. There is no environment override or unlimited fallback.

Daemon startup creates one `mv::allocation::AllocationBudget` before snapshot
restore. Public snapshot readers require its reference; `SnapshotMaker` retains
that original pool and passes a clone of its handle through each writer operation.
Core uses `mv::allocation::ChargedByteBuffer` directly. That safe public owner
reserves its checked exact `Layout::array::<u8>`
before asking the global allocator for storage. Its private `Vec` retains that
same pointer, capacity and allocator through every bounded append. Length starts
at zero; appends initialize bytes before exposing them. A zero-sized buffer
requests no backing allocation. Null allocation returns an explicit local error.
Field drop order deallocates the Vec before dropping its original charge.

Both payload consumers keep this owner alive through their complete validation.
The low-level reader reserves before seeking or reading the retained descriptor.
Strict restore retains the buffer through State initialization and Kura evidence
reconciliation. GC retains it through Merkle and stable-file validation.

## Refusal and notification

`TryReadError` and `TryWriteError` distinguish original pool admission refusal from
allocator failure and malformed snapshot evidence. A capacity error retains its
original release observation. GC candidate filtering propagates local allocation
errors rather than treating the generation as invalid or omitting a valid
rollback predecessor. Planning completes before current-pointer replacement or
GC deletion. An immutable newly staged generation can remain for a later retry;
the previous authoritative pointer and rollback generation remain intact.

Daemon startup never converts these local allocation failures into permission to
rebuild an empty State. Snapshot creation reports failure without advancing its
last-published block marker, so a later writer attempt remains eligible.

Refund notifications are deferred across the complete synchronous read or writer
operation. The writer acquires and releases its publication guard inside that
scope, including error and unwind paths. Backing bytes are physically freed and
credits returned before notification; callbacks run only after operation-owned
publication locks release. These scopes do not cover caller-owned outer guards
or asynchronous work.

## Controls and qualification status

The explicit MV `charged_byte_buffer_custody` integration target uses the
production `mv::allocation::ChargedByteBuffer` API directly. Its unsafe allocator
boundary lives only in MV; Core retains its unsafe-code prohibition. Its allocator observer checks exact layout,
pointer stability, deallocation before refund/wake, zero and overflow demand,
capacity refusal before allocation, original-release retry, allocator failure,
unwind and simultaneous buffer owners. Snapshot unit tests check retained file
position on refusal, source integrity, charged State initialization, original pool
handoff to SnapshotMaker, GC pointer/rollback preservation and retry, fallible GC
fallback selection, and callbacks after publication unlock on success/error and
unwind. Config and daemon controls cover defaults, limits and recovery policy.

Combined production check 6 rejected the initial Core-local allocator boundary
under Core's unsafe-code prohibition. That implementation and its Core test target
were removed in favor of the single MV owner above; the failing check remains
evidence and no lint was waived.

At source freeze, focused rustfmt, `git diff --check`, and
`scripts/check_no_legacy_codec.sh` passed. The integration owner ran `cargo test -p mv --test charged_byte_buffer_custody
-- --nocapture`: all six tests passed, with zero ignored. That pass is limited
to the actual byte owner and allocator controls. The full `iroha_config_integration` suite subsequently passed 241 tests with
zero ignored, including both snapshot limit controls and the snapshot config
fixture. Its initial test-helper Result-alias compile failure is preserved;
the correction only spells `std::result::Result<Config, String>` explicitly.
Core snapshot and daemon runtime controls remain queued. The integration owner
reports combined production check 8 passed before later unrelated edits; that
is scoped compilation evidence, not a completed release matrix.
The source manifest and requested commands are captured under
`target/first-release-snapshot-read-buffer-custody-20260920/` (untracked evidence).

The canonical remaining test selectors are:

```sh
cargo test -p iroha_core --lib snapshot::tests::snapshot_read_buffer_ -- --nocapture
cargo test -p iroha_core --lib snapshot::tests:: -- --nocapture
cargo test -p iroha_config --test iroha_config_integration fixtures::snapshot_read_buffer_tests -- --nocapture
cargo test -p iroha_config --test iroha_config_integration fixtures::minimal_config_snapshot -- --nocapture
cargo test -p irohad --lib snapshot_read_error_tests -- --nocapture
```

The focused Core selector names eight tests. Config disables automatic test
harness discovery, and the daemon library includes the main implementation; the
config `fixtures` and daemon `iroha3d` targets previously requested were not the
canonical owners. Preserve nonzero selected counts when recording these results.
