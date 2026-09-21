# DPN live recovery — September 21, 2026

## Admitted insertion transactions

The existing explicit prepaid MV Storage family now lends its original current
and block-undo checkpoints to an insertion transaction. The canonical pair callback
borrows the incoming key so one checked demand includes both tree edits, real
sorted touch-array growth and the current policy's nested key copy. One original
finite reservation supplies all three owners. Duplicate touches preserve their
first owned key; no-op insertions remain in ordered, allocation-free touch reads.
Preparation leaves the original touch set unchanged. Installation moves existing
keys and returns the emptied old array for cleanup while aggregate failure stays
armed. Key cleanup drains the remaining initialized prefix on a first destructor
panic; actual array deallocation precedes credit refund. Child abort restores
both parent roots, and a caught edit or cleanup panic makes the original block
unusable before partial publication. The original Untracked Storage path remains
in the same family. Removal, mutable replacement and detached capture are outside
this admitted insertion slice.

This is component implementation, not full State resource admission or release
qualification. World remains Untracked; concrete model policies, native control
and publication storage, aggregate admission, original Validate-to-Apply custody
and the native producer cutover remain required. Candidate30301 retains its failed
four-validator qualification: the retired MergeQC producer source was correctly
rejected by the ordinary execution owner. This local change performs no deployment
or ledger action. The [September 20 record](../2026-09-20/dpn-live-recovery.md)
retains the earlier incident and component scopes.

## Validation and retained failure

Before the two native compile corrections, the maintained gate passed 220 Python
tests on macOS and 218 executed cases with two platform skips on native Linux.
Its scoped source hashes remained unchanged. The earlier paired receipt is `target/dpn-devex/admitted-transaction-gate-20260920/paired-python-220-complete.json`,
SHA256 `37679532a5ca145c9c006288f830645811647ace8ab3fcf778fa012495ca8bcd`.

Native attempt1 failed its early test-metadata check after 2.1 seconds with E0283
at `touches.rs`: raw-pointer target inference could not choose the drain's key
type. The correction explicitly constructs `Drain::<K>` from `Buffer<K>`. It
changes no layout, initialized-prefix ownership, destructor order or refund
semantics. The failed command receipt remains
`target/dpn-devex/native-storage-transaction-focus-20260921-attempt1-command.json`,
SHA256 `69ef6d32c2b5494715a3470df121f3f937d0df3a9413121906dd576703ca16c8`;
its before/after source and HEAD checks agree. The independent annotation review
is `target/dpn-devex/mv-touch-drain-inference-review-20260921.json`, SHA256
`7ceb3c796b8f3764c21824f95a1663745100785d5bc1e2a93ea9b8088aad0b9a`.

Native attempt2 closed with exit1 after the wider Core test-metadata build
exposed five diagnostics from one missing trait import: `StorageReadOnly` was no
longer inherited from a production parent through `use super::*`. Historical
recovery and ordinary-to-autonomous test fragments therefore could not resolve
`iter`/`get`, with two consequent type-inference diagnostics. The explicit import
now lives only in `sumeragi/v2_apply_tests.rs`, whose parent registers it under
`#[cfg(test)]`; no production import or runtime guard changed. The failed metadata
phase took 213.2 seconds. Its unchanged-source/HEAD command receipt is
`target/dpn-devex/native-storage-transaction-focus-20260921-attempt2-command.json`,
SHA256 `525c687a99ea944b9759066e6f0dd5850e5b5d34a4bb42bc11b4cd3ac6a2992f`.
Both failed attempts remain recorded; neither executed the selected runtime census.

After both corrections, the paired gate again passes all 220 tests on macOS
(3.849 seconds) and 218 executed cases with two skips on native Linux (2.629
seconds). All 22 scoped files are unchanged and agree across hosts, including
the explicit Core test import. The current paired receipt is
`target/dpn-devex/admitted-transaction-gate-20260920/attempt3/paired-python-220-complete.json`,
SHA256 `500beafc262f4692a95ea32333828ad6d501994c26cc01e492b3ad41909f1bda`.
That receipt covers the Python source scope at that stage; later corrections have separate receipts below.

Native attempt3 closed with exit1 on unchanged source and HEAD after all ten
requested harnesses passed metadata (117.8 seconds) and code generation (382.8
seconds), and all four mandatory configuration controls passed. Before any of the
236 selected regressions ran, the exact native harness census refused the stale
registered name `release::tests::storage_first_undo_clone_panic_wakes_an_already_registered_retry`.
The current test is `release::tests::storage_revert_preimage_clone_panic_wakes_an_already_registered_retry`.
This is a selector-registration defect; successful compilation and configuration
do not establish a passing runtime selection. The retained command receipt is
`target/dpn-devex/native-storage-transaction-focus-20260921-attempt3-command.json`,
SHA256 `4487428014a7fd8f26a0016d7c2ab1905677bf9f8acb4e8d1d9000e0c3224d89`.
The corrected gate retains the same census count and adds a bounded early
lexical audit of the six explicit flat MV test modules, their declared source
edges and selected test names before Cargo. This source-name check does not
replace the authoritative compiled harness census. The gate SHA256 is
`ac3720dc9945efb133000b2152789ad650b0e4c7c73864cae676ca562df914c1`;
its Python test SHA256 is
`7405c7bff174b46db7c300e6c3fa6d9060ba5b0b56953c29023862093b72d595`.
The subsequent paired run passes 220 tests on macOS (4.346 seconds) and 218
executed cases plus two platform skips on Linux (3.147 seconds), with all 30
scoped source files unchanged and identical across hosts. Receipt:
`target/dpn-devex/mv-selector-source-audit-20260920/paired/paired-python-220-complete.json`,
SHA256 `40a1e8f6cfbedb11fbb08b07853d64ee01b868b77c29f44e7953b37e036bfccf`.

Native attempt4 closed with exit1 on unchanged source and HEAD. The same warm
lane passed metadata in 1.3 seconds and code generation in 1.2 seconds, then all
four configuration controls. It executed all 236 selected regressions: 234 passed
and two new Transaction pointer assertions failed:

- `storage_custody::actual_transaction_ordered_unique_touches_preserve_noop_and_sibling_preimages`
- `storage_custody::actual_transaction_full_budget_abort_restores_parent_and_outer_abort_preserves_readers`

The exact retained command receipt is
`target/dpn-devex/native-storage-transaction-focus-20260921-attempt4-command.json`,
SHA256 `5d1f8a3a0633d85a73f6bbf77265acbc51c441e80a39ad02c2c66a0125a2dc79`.
Review traced both assertions to legitimate undo-node copy-on-write when a later
operation inserts another key. The applied fixture-only correction first checks
that repeating key7 preserves its original first-preimage pointer, before adding
key2/key9. The new-key operation then requires a distinct actual undo allocation
record and pointer, unchanged first-preimage bytes, and still-live, unrefunded
custody. The saved new pointer is checked across child apply. Abort, refusal,
original-reader and pool assertions remain intact; no production Rust changed.
The repair is `target/dpn-devex/transaction-undo-cow-fixture-20260921/repair.patch`,
SHA256 `8a4ea44700f398f49b7fc4c9823097c20ca0a2154dd409216b6d17e61a97a862`.
This failed diagnostic remains recorded separately from the corrected attempt5 below.

The corrected native selection contains exactly 236 controls: 49 MV library,
22 original-map, 38 admitted-map, 45 Concread, 78 Core, and one each for CLI,
wallet, client and data-model. Four mandatory configuration controls run separately.
The 49 MV controls retain all 43 prior registered library cases plus six new
touch cases. Five additional actual-Storage transaction cases complete the eleven
new controls listed below.

Corrected attempt5 passed all 236 selected runtime controls and four mandatory
configuration controls, with zero failures. Tracked and nonignored source files,
index, branch and HEAD were identical before and after the run. Metadata took
1.3 seconds, code generation 1.7 seconds, and the complete diagnostic 131.825
seconds in the same warm Linux target. These are focused mutable diagnostic
results, not immutable release qualification or evidence of a live deployment.

The completed receipt is
`target/dpn-devex/native-storage-transaction-validation-complete-20260921.json`,
SHA256 `f51597611388a20924918462b2ea28e00638108841e4a27eaad925b5bf320559`.
It retains the exact command/source manifests, native artifact hashes, compile
events and all four preceding failed attempts.

After the fixture correction, the maintained Python gate passes 220 tests on
macOS (4.275 seconds) and 218 executed cases with two platform skips on Linux
(3.083 seconds). All 30 scoped source files are unchanged and identical across
hosts. Receipt:
`target/dpn-devex/transaction-undo-cow-fixture-20260921/paired/paired-python-220-complete.json`,
SHA256 `4dcb3f58a30eeddf992f9ef59be79895556b6b9a47f0d7250886a2d368d4960f`.

Only this dated record and the current status paragraph changed after native
validation. No Rust implementation, test body or checker logic changed afterward.

The eleven newly selected controls are:

- `mv=storage::touches::tests::touch_sorted_unique_growth_moves_original_key_allocations_without_copying`
- `mv=storage::touches::tests::touch_exact_joined_capacity_and_one_byte_below_preserve_original_state`
- `mv=storage::touches::tests::touch_preparation_abandonment_and_copy_panic_leave_old_array_and_keys_exact`
- `mv=storage::touches::tests::touch_arbitrary_key_drop_panic_drains_remaining_prefix_and_refunds_real_owners`
- `mv=storage::touches::tests::touch_refused_payload_and_checked_growth_overflow_allocate_nothing`
- `mv=storage::touches::tests::touch_plan_extends_original_demand_and_preserves_exact_provider_remainder`
- `mv-admitted-map=storage_custody::actual_transaction_joined_touch_and_pair_refusal_preserves_inputs_for_exact_retry`
- `mv-admitted-map=storage_custody::actual_transaction_ordered_unique_touches_preserve_noop_and_sibling_preimages`
- `mv-admitted-map=storage_custody::actual_transaction_full_budget_abort_restores_parent_and_outer_abort_preserves_readers`
- `mv-admitted-map=storage_custody::actual_transaction_caught_touch_and_pair_copy_panics_cannot_apply_or_publish`
- `mv-admitted-map=storage_custody::actual_transaction_touch_destructor_panic_cannot_apply_or_publish`

## Previous current status, preserved verbatim

Current DPN DevEx work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. The latest unchanged-source Linux diagnostic passes all 183 selected native controls and four mandatory configuration controls: 101 storage, 77 Core connection/publication, four client/model and one public-contract fee test. The original MV Storage family now supports explicit prepaid insertion blocks, one physical allocation pool, jointly admitted startup, real undo reset, retained first preimages and whole-block refund deferral. Eight actual Storage controls cover refusal/retry, reader retention, undo history and caught edit panic before publication. These changes do not activate prepaid World execution. The maintained gate passes 220 Python tests on Mac and 218 executed cases with two macOS-only skips on Linux. Candidate30301 remains the failed four-validator release: the old producer emits a retired MergeQC source that the ordinary execution owner correctly rejects. Transaction/control/model allocation admission, original Validate-to-Apply custody and one native producer cutover remain required before new immutable release qualification and DPN deployment. See the [incident and validation record](docs/history/2026-09-20/dpn-live-recovery.md).
