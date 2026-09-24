# F02 penalty share-key ownership, 2026-09-24

Scope: the existing `optimizations` checkout. This is one memory-ownership cut,
not complete penalty-preparation resource admission.

The parent penalty scan already builds a `PublicLaneStakeIndex` containing each
validator's canonical stake-share keys. It then copied every active validator's
full key slice into its `ValidatorLocator` and dropped the index. That second
inventory could duplicate a maximum-shape retained stake table during every
penalty preparation. The snapshot now keeps the original index as its sole
share-key owner. Locators retain validator identity, tenure and frozen exposure;
the action builder borrows the exact key slice from the same snapshot index for
exposure recomputation and slash planning. No stake arithmetic, action order or
wire representation changes.

Validation on this source: `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p
iroha_core --lib parent_penalty_plan_retains_only_due_evidence_metadata --
--nocapture` passed 1/1. The resulting Core test binary passed all 24 penalty
tests and 38 evidence tests. `cargo fmt --all -- --check`,
`python3 scripts/check_ivm_only.py`, and `git diff --check` passed.

The index's own `BTreeMap` and nested `Vec` allocations, validator
locator map, scratch block, action vector, and consensus-effects transactions
are still not funded by an original State-owned preparation reservation. F02
also needs capacity refusal routed as a local retry and restart/lifecycle tests.
