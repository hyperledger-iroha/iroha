# Kura / WSV Security & Performance Audit (2026-02-19)

## Scope

This audit covered:

- Kura persistence and budget paths: `crates/iroha_core/src/kura.rs`
- Production WSV/state commit/query paths: `crates/iroha_core/src/state.rs`
- IVM WSV mock host surfaces (test/dev scope): `crates/ivm/src/mock_wsv.rs`

Out of scope: unrelated crates and full-system benchmark reruns.

## Risk Summary

- Open Critical: 0
- Open High: 0
- Open Medium: 0
- Open Low: 0
- Addressed High: 4
- Addressed Medium: 6
- Addressed Low: 3

## Findings (Ordered by Severity)

### High

1. **Kura writer records recoverable I/O faults instead of panicking**
- Component: Kura
- Type: Security (DoS), Reliability
- Status: addressed for background writer fsync failures and synchronous append
  failure propagation.
- Detail: the writer loop records periodic/shutdown fsync failures through
  `writer_fault` and returns without unwinding; synchronous append failures
  continue to propagate as `Result` errors without exposing partial blocks.
- Evidence:
  - Writer fault state and recorder: `crates/iroha_core/src/kura.rs:236`, `crates/iroha_core/src/kura.rs:1689`
  - Periodic/shutdown fsync fault capture: `crates/iroha_core/src/kura.rs:2777`, `crates/iroha_core/src/kura.rs:2804`
  - Writer-channel and writer-fault regressions: `crates/iroha_core/src/kura.rs:10854`, `crates/iroha_core/src/kura.rs:10864`
  - Commit-marker and periodic-fsync regressions: `crates/iroha_core/src/kura.rs:16553`, `crates/iroha_core/src/kura.rs:16594`
- Impact: transient writer fsync faults degrade persistence health instead of
  killing the process.

2. **Kura eviction does full data/index rewrites under `block_store` mutex**
- Component: Kura
- Type: Performance, Availability
- Status: addressed for the `block_store` mutex hold and over-budget caller
  paths. Explicit maintenance calls still use the same synchronous compaction
  implementation under the write gate.
- Detail: `evict_block_bodies` now snapshots durable metadata while holding
  `block_store`, releases that mutex while copying DA sidecars and compacting
  `blocks.data`/`blocks.index` into temp files, then reacquires `block_store`
  only for final validation, atomic replacement, directory sync, and cache
  refresh. A separate `block_store_write_lock` serializes appends, prunes, and
  rehydration while allowing readers that only need `block_store` metadata to
  proceed during long compaction.
- Evidence:
  - Write gate field: `crates/iroha_core/src/kura.rs:159`
  - Eviction write-gate acquisition: `crates/iroha_core/src/kura.rs:1703`, `crates/iroha_core/src/kura.rs:1708`
  - Temp-file data/index replacement: `crates/iroha_core/src/kura.rs:1887`, `crates/iroha_core/src/kura.rs:1889`
  - Writer/rehydration/prune serialization: `crates/iroha_core/src/kura.rs:2978`, `crates/iroha_core/src/kura.rs:3803`, `crates/iroha_core/src/kura.rs:4813`
  - Regression test: `crates/iroha_core/src/kura.rs:12003`
  - Long-history eviction seeding regression:
    `bench_eviction_helpers_seed_remote_eviction`
    (`crates/iroha_core/src/kura.rs:12060`)
  - Criterion long-history compaction benchmark:
    `kura_eviction_long_history_compaction`
    (`crates/iroha_core/benches/kura.rs:260`)
- Impact: large-history eviction no longer monopolizes `block_store` during
  the full data/index rewrite, reducing read-side stalls during compaction.

3. **State commit holds coarse `view_lock` across heavy commit work**
- Component: Production WSV
- Type: Performance, Availability
- Status: addressed.
- Detail: block, merge-ledger, and lane-lifecycle commit paths no longer use
  the reader/writer `view_lock` for heavy multi-component writes. They now
  serialize writers with a dedicated `state_write_lock` and use the even/odd
  state-view generation to make full `State::view` retry while a writer is
  active.
- Evidence:
  - Writer serialization lock: `crates/iroha_core/src/state.rs:7337`
  - StateBlock writer lock reference: `crates/iroha_core/src/state.rs:7443`
  - Generation guard: `crates/iroha_core/src/state.rs:21512`
  - Merge-ledger commit writer lock: `crates/iroha_core/src/state.rs:22951`
  - Lane lifecycle writer lock: `crates/iroha_core/src/state.rs:23574`
  - Block commit writer lock: `crates/iroha_core/src/state.rs:27550`
  - State-write-lock telemetry with legacy view-lock aliases: `crates/iroha_core/src/telemetry.rs:716`, `crates/iroha_telemetry/src/metrics.rs:7111`
  - Lock-order regressions: `crates/iroha_core/src/state.rs:29057`, `crates/iroha_core/src/state.rs:29097`
  - Heavy-world commit benchmark helper and regression:
    `commit_heavy_world_accounts_for_bench` and
    `heavy_world_commit_bench_helper_commits_accounts`
    (`crates/iroha_core/src/state.rs:20443`,
    `crates/iroha_core/src/state.rs:29181`)
  - Criterion heavy-world state-write-lock benchmark:
    `state_write_lock_heavy_world_commit`
    (`crates/iroha_core/benches/state.rs:14`)
- Impact: heavy commit phases no longer occupy a `view_lock`; full state
  views coordinate through generation retry, and writer-vs-writer ordering is
  explicit.

4. **IVM WSV admin aliases and direct admin syscalls require caller checks (test/dev host)**
- Component: IVM WSV mock host
- Type: Security (Privilege escalation in test/dev environments)
- Status: addressed.
- Detail: JSON admin aliases and direct role/permission syscalls now require
  caller-scoped management tokens before mutating administrative WSV state:
  `ManageRoles` for role create/delete/grant/revoke, `ManagePermissions` for
  direct permission grant/revoke, and `ManagePeers` for peer mutation aliases.
- Evidence:
  - Role alias gates: `crates/ivm/src/mock_wsv.rs:5249`, `crates/ivm/src/mock_wsv.rs:5301`, `crates/ivm/src/mock_wsv.rs:5320`, `crates/ivm/src/mock_wsv.rs:5344`
  - Permission alias gates: `crates/ivm/src/mock_wsv.rs:5368`, `crates/ivm/src/mock_wsv.rs:5394`
  - Peer alias gates: `crates/ivm/src/mock_wsv.rs:5473`, `crates/ivm/src/mock_wsv.rs:5489`
  - Direct syscall gates: `crates/ivm/src/mock_wsv.rs:6001`, `crates/ivm/src/mock_wsv.rs:6033`, `crates/ivm/src/mock_wsv.rs:6053`, `crates/ivm/src/mock_wsv.rs:6083`, `crates/ivm/src/mock_wsv.rs:6098`, `crates/ivm/src/mock_wsv.rs:6114`
  - Regression tests: `crates/ivm/src/mock_wsv.rs:7657`, `crates/ivm/src/mock_wsv.rs:7710`, `crates/ivm/tests/wsv_host_roles_triggers_envelope.rs:283`
  - Scope note in file docs (test/dev intent): `crates/ivm/src/mock_wsv.rs:295`
- Impact: unauthorized test contracts/tools cannot self-elevate through the
  mock-host admin surfaces.

### Medium

5. **Kura budget checks cache pending and durable metadata instead of re-reading/re-encoding on every check**
- Component: Kura
- Type: Performance
- Status: addressed for repeated pending-block re-encoding and repeated
  durable block-store metadata reads; storage-budget eviction is now scheduled
  for background maintenance instead of compacting on the caller path.
- Detail: pending block bytes are cached in `pending_budget_bytes` after the
  first calculation and invalidated when the in-memory block set changes.
  Durable budget metadata is cached in a small snapshot and refreshed by
  successful appends, prunes, evictions, and disk-usage refreshes.
- Evidence:
  - Pending cache fields: `crates/iroha_core/src/kura.rs:204`, `crates/iroha_core/src/kura.rs:206`
  - Test-only raw pending scan counter: `crates/iroha_core/src/kura.rs:209`, `crates/iroha_core/src/kura.rs:4433`
  - Durable snapshot fields: `crates/iroha_core/src/kura.rs:213`, `crates/iroha_core/src/kura.rs:217`
  - Durable snapshot helpers: `crates/iroha_core/src/kura.rs:1658`, `crates/iroha_core/src/kura.rs:1668`
  - Raw metadata fallback and cached read path: `crates/iroha_core/src/kura.rs:4472`, `crates/iroha_core/src/kura.rs:4497`
  - Snapshot publication after appends/eviction/prune: `crates/iroha_core/src/kura.rs:1907`, `crates/iroha_core/src/kura.rs:3850`, `crates/iroha_core/src/kura.rs:4840`
  - Cache regressions:
    `durable_budget_snapshot_avoids_repeated_metadata_reads` and
    `kura_budget_check_scales_with_pending_depth`
    (`crates/iroha_core/src/kura.rs:10069`,
    `crates/iroha_core/src/kura.rs:10113`)
  - Criterion pending-depth benchmark:
    `kura_storage_budget_cached_pending_depth`
    (`crates/iroha_core/benches/kura.rs:194`,
    `crates/iroha_core/benches/kura.rs:195`)
- Impact: repeated budget checks no longer re-encode the same pending blocks or
  lock/read `block_store` metadata while the durable snapshot remains valid.

6. **Kura budget checks avoid repeated block-store metadata reads per enqueue**
- Component: Kura
- Type: Performance
- Status: addressed by the durable budget snapshot described in finding 5.
- Detail: budget checks first read cached durable count/unindexed-byte metadata
  and only fall back to durable index/file-length reads after invalidation or
  explicit disk-usage refresh.
- Evidence:
  - Cached read path: `crates/iroha_core/src/kura.rs:4497`
  - Raw fallback path: `crates/iroha_core/src/kura.rs:4472`
  - Regression test: `crates/iroha_core/src/kura.rs:10069`
- Impact: avoidable I/O/lock overhead is removed from the hot enqueue path
  when Kura has a valid durable snapshot.

7. **Kura eviction is triggered inline from enqueue budget path**
- Component: Kura
- Type: Performance, Availability
- Status: addressed.
- Detail: over-budget enqueue/replace checks now coalesce the required reclaim
  bytes into `pending_budget_eviction_bytes`, wake the Kura writer with a
  storage-budget eviction notification, and fail fast with
  `StorageBudgetExceeded`. The writer maintenance loop drains the pending
  request and runs `evict_block_bodies` outside the caller path; callers can
  retry after maintenance reclaims replicated bodies.
- Evidence:
  - Pending request field: `crates/iroha_core/src/kura.rs:211`
  - Writer notification variant: `crates/iroha_core/src/kura.rs:778`
  - Coalesced request and maintenance drain: `crates/iroha_core/src/kura.rs:1606`, `crates/iroha_core/src/kura.rs:1630`
  - Writer loop maintenance hook: `crates/iroha_core/src/kura.rs:2800`
  - Budget checks schedule instead of compacting inline: `crates/iroha_core/src/kura.rs:4562`, `crates/iroha_core/src/kura.rs:4660`
  - Regression test: `crates/iroha_core/src/kura.rs:10339`
- Impact: near-budget block ingest no longer performs full block-body
  compaction on the foreground caller path while still failing closed until
  background maintenance has actually reclaimed disk budget.

8. **`State::view` retries instead of returning mixed snapshots under writer contention**
- Component: Production WSV
- Type: Consistency/Performance tradeoff
- Status: addressed.
- Detail: full state views now use an even/odd commit generation. Writers mark
  the generation active while mutating multi-component state, and `State::view`
  drops partial snapshots and retries if the generation is active or changes
  during acquisition.
- Evidence:
  - Generation guard: `crates/iroha_core/src/state.rs:7160`
  - Stable-generation retry: `crates/iroha_core/src/state.rs:21555`
  - Writer markings: `crates/iroha_core/src/state.rs:22953`,
    `crates/iroha_core/src/state.rs:23608`,
    `crates/iroha_core/src/state.rs:27552`
  - Regression test: `crates/iroha_core/src/state.rs:29007`
- Impact: full state views no longer return a known mixed component snapshot
  when writer contention is observed; query-oriented snapshots remain available
  for callers that explicitly accept the lighter consistency contract.

9. **`apply_without_execution` records DA cursor advancement errors without panicking**
- Component: Production WSV
- Type: Security (DoS via panic-on-invariant-break), Reliability
- Status: addressed.
- Detail: committed block apply records shard cursor advancement failures,
  logs receipt/confidential-compute indexing failures, and continues applying
  the committed block without unwinding.
- Evidence:
  - `crates/iroha_core/src/state.rs:27039`
  - `crates/iroha_core/src/state.rs:27753`
  - `crates/iroha_core/src/state.rs:27756`
  - `crates/iroha_core/src/state.rs:47828`
- Impact: latent DA cursor validation/indexing bugs are surfaced through
  telemetry/logging instead of becoming node-killing failures.

10. **IVM TLV publish and JSON envelope paths enforce explicit size bounds**
- Component: IVM WSV mock host
- Type: Security (memory DoS), Performance
- Status: addressed in the test/dev host.
- Detail: TLV publish checks INPUT-resident payload length and raw-header total
  envelope size against `max_envelope_bytes` before copying; JSON instruction
  envelopes reject oversized payloads before decode.
- Evidence:
  - `crates/ivm/src/mock_wsv.rs:4642`
  - `crates/ivm/src/mock_wsv.rs:4665`
  - `crates/ivm/src/mock_wsv.rs:4957`
  - `crates/ivm/src/mock_wsv.rs:7542`
  - `crates/ivm/src/mock_wsv.rs:7795`
- Impact: malformed test payloads are bounded before allocation-heavy decode.

### Low

11. **Kura notify wake signaling is now coalesced (`std::sync::mpsc::sync_channel`)**
- Component: Kura
- Type: Performance/Memory hygiene
- Status: addressed for writer wake events.
- Detail: writer wake signaling now uses a capacity-one synchronous channel and
  non-blocking `try_send`; redundant wake events are coalesced when a wake is
  already pending. Pipeline sidecars only notify on an empty-to-nonempty queue
  transition, while shutdown still wakes the writer through the same bounded
  signal path.
- Evidence:
  - `crates/iroha_core/src/kura.rs:160`
  - `crates/iroha_core/src/kura.rs:741`
  - `crates/iroha_core/src/kura.rs:5447`
- Impact: redundant wake-event memory growth is capped; pipeline sidecar queue
  growth is capped separately below.

12. **Pipeline sidecar queue is capped by configured Kura memory depth**
- Component: Kura
- Type: Performance/Memory hygiene
- Status: addressed.
- Detail: pipeline sidecar enqueue now checks a cap derived from
  `kura.blocks_in_memory`, accepts only while the pending writer queue is below
  that cap, and returns an explicit `RejectedQueueFull` result for callers to
  log under sustained writer backlog. The sidecar remains best-effort recovery
  metadata; consensus execution is not blocked by sidecar persistence pressure.
- Evidence:
  - `crates/iroha_core/src/kura.rs:171`
  - `crates/iroha_core/src/kura.rs:5469`
  - `crates/iroha_core/src/block.rs:8668`
- Impact: a stalled writer thread has bounded sidecar-queue memory exposure.

13. **IVM WSV host checkpoint avoids a duplicate durable-state snapshot**
- Component: IVM WSV mock host
- Type: Performance/Memory hygiene
- Status: addressed in the test/dev host.
- Detail: `WsvHostSnapshot` now relies on the cloned `MockWorldStateView` for
  durable smart-contract state instead of carrying a second
  `DurableStateSnapshot`. Restore flushes the cloned WSV overlay so persisted
  state-store rollback semantics are preserved without cloning the durable map
  twice.
- Evidence:
  - Snapshot shape: `crates/ivm/src/mock_wsv.rs:1974`
  - Checkpoint and restore paths: `crates/ivm/src/mock_wsv.rs:2109`,
    `crates/ivm/src/mock_wsv.rs:2135`,
    `crates/ivm/src/mock_wsv.rs:2137`
  - Persisted restore regression:
    `checkpoint_restore_flushes_persisted_wsv_state`
    (`crates/ivm/tests/wsv_state_overlay.rs:167`)
  - Criterion checkpoint/restore benchmark:
    `mock_wsv_checkpoint_restore`
    (`crates/ivm/benches/bench_wsv.rs:235`,
    `crates/ivm/benches/bench_wsv.rs:261`)
- Impact: block-level VM rollback checkpoints no longer duplicate the durable
  smart-contract state map while still rolling persisted test state back to the
  checkpoint contents.

## Existing Test Coverage and Gaps

### Kura

- Existing coverage:
  - storage-budget behavior: `store_block_rejects_when_budget_exceeded`, `store_block_rejects_when_pending_blocks_exceed_budget`, `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`, `crates/iroha_core/src/kura.rs:6949`, `crates/iroha_core/src/kura.rs:6984`)
  - eviction correctness and rehydration: `evict_block_bodies_does_not_truncate_unpersisted`, `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`, `crates/iroha_core/src/kura.rs:8126`)
  - writer fault isolation and fsync regression coverage:
    `store_block_does_not_depend_on_writer_channel`,
    `store_block_does_not_depend_on_writer_fault`,
    `commit_marker_write_failure_keeps_pending`, and
    `writer_loop_records_periodic_fsync_failure_without_panic`
  - storage-budget cache coverage:
    `durable_budget_snapshot_avoids_repeated_metadata_reads` and
    `kura_budget_check_scales_with_pending_depth`
  - pending-depth storage-budget benchmark coverage:
    `kura_storage_budget_cached_pending_depth`
  - long-history eviction helper and benchmark coverage:
    `bench_eviction_helpers_seed_remote_eviction` and
    `kura_eviction_long_history_compaction`
  - background eviction retry-latency guardrail:
    `kura_background_eviction_retry_latency_threshold`
- Gaps: no current Kura security or hot-path performance gap remains open in
  this audit; operational latency SLOs should continue to be exercised by
  release/performance jobs.

### Production WSV

- Existing coverage:
  - state-view generation retry behavior:
    `state_view_waits_for_active_view_generation`
  - lock-order safety around tiered backend:
    `state_commit_does_not_hold_tiered_backend_while_waiting_for_state_write_lock`
  - apply-path DA cursor fault isolation:
    `apply_without_execution_records_da_cursor_errors_without_panic`
  - heavy-world commit helper and benchmark coverage:
    `heavy_world_commit_bench_helper_commits_accounts` and
    `state_write_lock_heavy_world_commit`
- Gaps: no current Production WSV security or hot-path performance gap remains
  open in this audit; release jobs should keep benchmark trends under review.

### IVM WSV Mock Host

- Existing coverage:
  - permission JSON parser semantics and peer parsing (`crates/ivm/src/mock_wsv.rs:5234`, `crates/ivm/src/mock_wsv.rs:5332`)
  - syscall smoke tests around TLV decode and JSON decode (`crates/ivm/src/mock_wsv.rs:5962`, `crates/ivm/src/mock_wsv.rs:6078`)
  - oversized TLV/JSON envelope rejection:
    `input_publish_tlv_rejects_oversized_envelope` and
    `execute_instruction_rejects_oversized_json_envelope`
  - admin alias and direct syscall authorization regression coverage:
    `envelope_admin_alias_rejects_without_manage_permissions`,
    `direct_admin_syscalls_require_management_permissions`, and
    `direct_admin_syscalls_succeed_with_management_permissions`
  - checkpoint/restore persisted-state coverage:
    `checkpoint_restore_flushes_persisted_wsv_state`
  - checkpoint/restore clone-cost benchmark coverage:
    `mock_wsv_checkpoint_restore`

## Prioritized Remediation Plan

### Phase 1 (High-impact hardening)

1. Replace Kura writer `panic!` branches with recoverable error propagation + degraded-health signaling.
- Status: addressed for writer fsync failures and synchronous block append
  fault isolation.
- Target files: `crates/iroha_core/src/kura.rs`
- Acceptance:
  - injected append/index/fsync failures do not panic
  - errors are surfaced through telemetry/logging and writer remains controllable

2. Add bounded envelope checks for IVM mock-host TLV publish and JSON envelope paths.
- Status: addressed.
- Target files: `crates/ivm/src/mock_wsv.rs`
- Acceptance:
  - oversized payloads are rejected before allocation-heavy processing
  - new tests cover both TLV and JSON oversized cases

3. Enforce explicit caller permission checks for JSON admin aliases and direct admin syscalls.
- Status: addressed for role, direct-permission, and peer admin surfaces.
- Target files: `crates/ivm/src/mock_wsv.rs`
- Acceptance:
  - unauthorized caller cannot mutate role/permission/peer state through aliases
    or direct role/permission syscalls

### Phase 2 (Hot-path performance)

4. Make Kura budget accounting incremental.
- Status: addressed for pending-block byte caching and durable block-store
  metadata snapshots; over-budget eviction is scheduled for background
  maintenance.
- Replace per-enqueue full pending-queue recomputation and repeated durable
  metadata reads with maintained counters/snapshots updated on enqueue/persist/drop.
- Acceptance:
  - enqueue cost near O(1) for pending-bytes calculation
  - hot budget checks avoid repeated block-store metadata reads
  - deterministic regression proves repeated budget checks over a deep pending
    queue avoid repeated raw pending scans
  - Criterion benchmark covers cached storage-budget checks across pending
    depths 0, 128, and 2,048

5. Reduce eviction lock hold time.
- Status: addressed for the `block_store` mutex hold by splitting eviction into
  a short metadata/final-swap critical section and temp-file compaction outside
  `block_store`; over-budget caller paths now schedule that maintenance rather
  than running it inline.
- Acceptance:
  - `block_store` is available while eviction is paused after its durable
    metadata snapshot
  - existing eviction idempotence and rehydration regressions continue to pass
  - long-history seeded-eviction regression and compaction benchmark cover the
    remote-replica advertisement path used by production body eviction

6. Shorten coarse `view_lock` critical section where feasible.
- Status: addressed by replacing commit-path `view_lock` usage with a
  writer-only `state_write_lock` plus state-view generation retry.
- Acceptance:
  - block, merge-ledger, and lane-lifecycle commits no longer acquire
    `view_lock`
  - existing state-view generation and writer lock-order regressions continue
    to pass
  - heavy-world commit benchmark covers account-heavy WSV mutations through the
    production commit path

### Phase 3 (Operational guardrails)

7. Introduce bounded/coalesced wake signaling for Kura writer and sidecar queue backpressure/caps.
- Status: addressed by capacity-one writer wake coalescing and a
  config-derived pipeline sidecar queue cap with explicit overflow rejection.
8. Expand telemetry dashboards for:
- `state_commit_write_lock_*` wait/hold distributions, while retaining legacy
  `state_commit_view_lock_*` aliases during dashboard migration
- eviction duration and reclaimed bytes per run
- budget-check enqueue latency

## Suggested Test Additions

- None at the audit source-coverage level. Keep release/performance jobs
  tracking Kura background-eviction retry latency and WSV heavy-commit
  benchmark trends over time.

## Notes on Scope and Confidence

- Findings for `crates/iroha_core/src/kura.rs` and `crates/iroha_core/src/state.rs` are production-path findings.
- Findings for `crates/ivm/src/mock_wsv.rs` are explicitly test/dev host scoped, per file-level documentation.
- No ABI versioning changes are required by this audit itself.
