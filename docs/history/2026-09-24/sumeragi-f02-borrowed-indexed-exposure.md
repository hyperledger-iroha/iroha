# F02 borrowed indexed exposure, 2026-09-24

Scope: the existing `optimizations` checkout. The read-only offence-height
exposure calculation in `smartcontracts/isi/staking.rs` now visits the exact
current-overlay stake-share rows through references from its indexed key slice.
It no longer builds a `Vec` of cloned share keys and full share records merely
to total bonded and eligible pending-unbond custody. The common exposure fold
accepts borrowed rows; the mutable slash path still retains its separate owned
update set. Canonical key ordering, validator ownership, storage-row binding,
and skipping a share consumed by an earlier ordered slash remain enforced.

The adjacent `staking_borrowed_exposure_tests.rs` exercises an accepted
offence-height boundary, an expired pending amount, unsorted and foreign keys,
a malformed final retained row, and an indexed key whose current-overlay share
has been removed. Direct `rustfmt --edition 2024 --check` on both changed Rust
files and `git diff --check` passed. The combined Core test binary's focused
`indexed_exposure_borrows_ordered_rows_and_skips_consumed_share` selector
passed 1/1. The full staking, penalty, and workspace suites were not run for
this slice.

This change removes a read-only scratch copy, not its surrounding allocation
obligations. `PublicLaneStakeIndex::from_world` still owns an unfunded B-tree,
per-validator vectors, cloned `AccountId` controllers and adaptive `Quantity`
values. The arithmetic fold still creates `Quantity` temporaries. Parent
penalty planning drops its `StateView` before constructing a scratch block, and
final application builds a second index from a mutable transaction overlay;
borrowed index keys cannot safely span either ownership boundary. The mutable
slash update set, State-owned reservation, local-retry routing, and full F02
qualification remain open.
