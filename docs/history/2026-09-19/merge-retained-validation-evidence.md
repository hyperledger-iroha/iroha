# Retained validation merge source evidence

These exact status paragraphs are historical evidence from the merge parents.
They do not qualify the combined candidate.

## Local parent `166c455c060a9dca193b0366b90a1b900676ba47`

The latest combined Core/Torii/test-network/Kagami/daemon build60 and all 1,256 selected runtime controls pass (1,203 Core, 53 Torii), with 7,060 captured inputs and emitted test binaries unchanged. One State-owned retained-admission authority now serves Queue refresh/retry/recovery and Native closed-lane opening; exact ranked pre-close bindings survive closure without reopening fresh or ordinary execution. All 362 Queue controls and eight new closure/restart cases are included. The canonical multilane binding gate and 117 focused Python controls pass on 8,138 unchanged inputs. The [retained-admission record](docs/history/2026-09-19/retained-admission-authority.md) preserves exact scopes and failed attempts. Earlier [physical Validate retry](docs/history/2026-09-19/physical-validation-waits.md) and [storage-verdict corrections](docs/history/2026-09-19/local-validation-verdicts.md) remain covered by the runtime selection. Torii empty-Queue canonical retry, exact terminal Queue cleanup, off-chain receipt carry/closure, complete capture admission, original Validate-to-Apply ownership, live shared-lane publication, participant durability, full workspace and unchanged four/seven-validator qualification remain open. No liveness goal is closed.

## Incoming parent `c8eae6b521cea6035fc34be10b403303ef771cb1`

The incoming source records an earlier conflict-resolution run with Core library compilation, 11 warnings and 239 selected native preparation/fixture source-contract tests across six independent shards. Its changed-file formatting, codec retirement, conflict/diff checks and historical archive verification passed, while workspace formatting reported differences in three unchanged files. Those results are parent-source evidence and do not qualify this merged candidate. The current merge resolution passes Core library and unit-test compilation, 211 focused runtime controls and 432 source/inventory/mutation controls. The runtime selection covers original carrier publication, source/archive authentication, native preparation, geometry retries, Queue retirement and cold pending-capacity accounting. The lease captures capacity before inner geometry/sidecar acquisition, preserves nonblocking cold-merge retry and prevents rescanning under held locks. Resolution-file formatting, codec retirement, conflict/diff checks and historical archive verification pass. Concurrent governance/data-model work is outside this focused behavioral qualification. Full workspace tests and unchanged four/seven-validator qualification remain outstanding. Production native runner cutover, original Validate-to-Apply custody, pre-vote capacity policy, pending geometry/Queue retirement, participant durability and carrier-proof custody remain separate open outcomes. Native DA/pin/SCCP payload execution remains unsupported; its complete owners, broader NPoS evidence/penalty and sponsor/lease coverage, aggregate resource admission and native Windows namespace durability remain outstanding. No L1–L6 goal is complete.

## Resolution scope

All 20 conflicting paths were resolved without compatibility aliases. Native
hard geometry and immutable local capacity remain distinct typed errors. Local
validation uses one refusal family and one move-only dispatch/retry owner.
Queue release observations share one predicate between synchronous publication
and the nonblocking retained cut, preserving their respective lock order.
The incoming retained geometry journal design remains authoritative.

The full multilane structural check initially reported 124 diagnostics after
resolving the include-provider index entries. Source and binding comparisons
against immutable incoming commit `c8eae6b521` identify 123 inherited diagnostics:
46 are duplicated whitespace-sensitive checks of otherwise identical incoming
bodies; the others still name moved or changed incoming State/Kura helpers.
The merge-specific retired `Queue::lane_has_pending_work_locked` binding was
removed, preserving its complete reservation and routing checks in the two
canonical predicate bindings. The reviewed include inventory now includes the
incoming `state/geometry_publication.rs`, with its canonical digest updated.
These comparisons are scoped baseline evidence, not a passing formal gate.

## Focused validation

- Core library compilation passed with warnings using
  `scripts/cargo_fast.sh --stable-local-metadata --incremental -- check --locked -p iroha_core --lib`.
- All 232 cases in
  `pytests/scripts/sumeragi_v2_multilane_native_preparation_contract_test.py`
  passed across six isolated shards plus a two-case rerun. The first run stopped
  one shard on a stale mutation anchor: adding the release-observation method
  made a generic field replacement target that method instead of the intended
  retirement predicate. The test now targets the exact predicate call; both
  that case and the unexecuted tail case pass without weakening assertions.
- `scripts/tests/sumeragi_source_contract_asset_compaction_test.py` passed
  six tests and 36 subtests, including the refreshed canonical asset pins.
- Scoped formatting passed for all 22 resolution Rust files. Workspace-wide
  formatting still reports two unchanged files outside this resolution.
- Codec-retirement checks and historical archive verification passed.
- The final multilane structural check reports 84 inherited diagnostics, all a
  subset of the compared incoming-parent failures. No new diagnostic remains
  from the Queue predicate conflict. Unrelated concurrent State/Kura guard
  corrections account for the reduced count; they are outside this resolution.

Core unit-test compilation passed with 44 warnings. Of 41 selected Rust cases,
39 passed on the default stack; two shared physical-validation fixtures
initially overflowed that stack, then passed unchanged with
`RUST_MIN_STACK=33554432`. The same helper contains the full launched-service
branch even when that branch is not taken. All three physical-retry cases now
use one explicit 32 MiB lifecycle-fixture wrapper, matching the convention
already used by their backpressure sibling and propagating all panics. The
final `cargo test --locked -p iroha_core --lib physical_validate_retry_ --
--test-threads=1` rerun is waiting for another Cargo build's shared artifact lock.
No Cargo process was interrupted. Full workspace and four/seven-validator
runtime qualification are not claimed.
