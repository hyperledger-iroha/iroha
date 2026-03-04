# Kura / WSV Security & Performance Audit (2026-02-19)

## Scope

This audit covered:

- Kura persistence and budget paths: `crates/iroha_core/src/kura.rs`
- Production WSV/state commit/query paths: `crates/iroha_core/src/state.rs`
- IVM WSV mock host surfaces (test/dev scope): `crates/ivm/src/mock_wsv.rs`

Out of scope: unrelated crates and full-system benchmark reruns.

## Risk Summary

- Critical: 0
- High: 4
- Medium: 6
- Low: 2

## Findings (Ordered by Severity)

### High

1. **Kura writer panics on I/O failures (node availability risk)**
- Component: Kura
- Type: Security (DoS), Reliability
- Detail: writer loop panics on append/index/fsync errors instead of returning recoverable errors, so transient disk faults can terminate the node process.
- Evidence:
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- Impact: remote load + local disk pressure can induce crash/restart loops.

2. **Kura eviction does full data/index rewrites under `block_store` mutex**
- Component: Kura
- Type: Performance, Availability
- Detail: `evict_block_bodies` rewrites `blocks.data` and `blocks.index` via temp files while holding `block_store` lock.
- Evidence:
  - Lock acquisition: `crates/iroha_core/src/kura.rs:834`
  - Full rewrite loops: `crates/iroha_core/src/kura.rs:921`, `crates/iroha_core/src/kura.rs:942`
  - Atomic replace/sync: `crates/iroha_core/src/kura.rs:956`, `crates/iroha_core/src/kura.rs:960`
- Impact: eviction events can stall writes/reads for prolonged periods on large histories.

3. **State commit holds coarse `view_lock` across heavy commit work**
- Component: Production WSV
- Type: Performance, Availability
- Detail: block commit holds exclusive `view_lock` while committing transactions, block hashes, and world state, creating reader starvation under heavy blocks.
- Evidence:
  - Lock hold begins: `crates/iroha_core/src/state.rs:17456`
  - Work inside lock: `crates/iroha_core/src/state.rs:17466`, `crates/iroha_core/src/state.rs:17476`, `crates/iroha_core/src/state.rs:17483`
- Impact: sustained heavy commits can degrade query/consensus responsiveness.

4. **IVM JSON admin aliases allow privileged mutations without caller checks (test/dev host)**
- Component: IVM WSV mock host
- Type: Security (Privilege escalation in test/dev environments)
- Detail: JSON alias handlers route directly to role/permission/peer mutation methods that do not require caller-scoped permission tokens.
- Evidence:
  - Admin aliases: `crates/ivm/src/mock_wsv.rs:4274`, `crates/ivm/src/mock_wsv.rs:4371`, `crates/ivm/src/mock_wsv.rs:4448`
  - Ungated mutators: `crates/ivm/src/mock_wsv.rs:1035`, `crates/ivm/src/mock_wsv.rs:1055`, `crates/ivm/src/mock_wsv.rs:855`
  - Scope note in file docs (test/dev intent): `crates/ivm/src/mock_wsv.rs:295`
- Impact: test contracts/tools can self-elevate and invalidate security assumptions in integration harnesses.

### Medium

5. **Kura budget checks re-encode pending blocks on each enqueue (O(n) per write)**
- Component: Kura
- Type: Performance
- Detail: each enqueue recomputes pending queue bytes by iterating pending blocks and serializing each via canonical wire size path.
- Evidence:
  - Queue scan: `crates/iroha_core/src/kura.rs:2509`
  - Per-block encode path: `crates/iroha_core/src/kura.rs:2194`, `crates/iroha_core/src/kura.rs:2525`
  - Called in budget check on enqueue: `crates/iroha_core/src/kura.rs:2580`, `crates/iroha_core/src/kura.rs:2050`
- Impact: write throughput degradation under backlog.

6. **Kura budget checks perform repeated block-store metadata reads per enqueue**
- Component: Kura
- Type: Performance
- Detail: each check reads durable index count and file lengths while locking `block_store`.
- Evidence:
  - `crates/iroha_core/src/kura.rs:2538`
  - `crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- Impact: avoidable I/O/lock overhead on hot enqueue path.

7. **Kura eviction is triggered inline from enqueue budget path**
- Component: Kura
- Type: Performance, Availability
- Detail: enqueue path can synchronously call eviction before accepting new blocks.
- Evidence:
  - Enqueue call chain: `crates/iroha_core/src/kura.rs:2050`
  - Inline eviction call: `crates/iroha_core/src/kura.rs:2603`
- Impact: tail-latency spikes on transaction/block ingest when near budget.

8. **`State::view` may return without acquiring coarse lock under contention**
- Component: Production WSV
- Type: Consistency/Performance tradeoff
- Detail: on write-lock contention, `try_read` fallback returns view without coarse guard by design.
- Evidence:
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
- Impact: improved liveness, but callers must tolerate weaker cross-component atomicity under contention.

9. **`apply_without_execution` uses hard `expect` on DA cursor advancement**
- Component: Production WSV
- Type: Security (DoS via panic-on-invariant-break), Reliability
- Detail: committed block apply path panics if DA cursor advancement invariants fail.
- Evidence:
  - `crates/iroha_core/src/state.rs:17621`
  - `crates/iroha_core/src/state.rs:17625`
- Impact: latent validation/indexing bugs can become node-killing failures.

10. **IVM TLV publish syscall lacks explicit envelope-size bound before allocation (test/dev host)**
- Component: IVM WSV mock host
- Type: Security (memory DoS), Performance
- Detail: reads header length then allocates/copies full TLV payload without a host-level cap in this path.
- Evidence:
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- Impact: malicious test payloads can force large allocations.

### Low

11. **Kura notify channel is unbounded (`std::sync::mpsc::channel`)**
- Component: Kura
- Type: Performance/Memory hygiene
- Detail: notification channel can accumulate redundant wake events during sustained producer pressure.
- Evidence:
  - `crates/iroha_core/src/kura.rs:552`
- Impact: memory growth risk is low per event size but avoidable.

12. **Pipeline sidecar queue is unbounded in memory until writer drain**
- Component: Kura
- Type: Performance/Memory hygiene
- Detail: sidecar queue `push_back` has no explicit cap/backpressure.
- Evidence:
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- Impact: potential memory growth during prolonged writer delays.

## Existing Test Coverage and Gaps

### Kura

- Existing coverage:
  - storage-budget behavior: `store_block_rejects_when_budget_exceeded`, `store_block_rejects_when_pending_blocks_exceed_budget`, `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`, `crates/iroha_core/src/kura.rs:6949`, `crates/iroha_core/src/kura.rs:6984`)
  - eviction correctness and rehydration: `evict_block_bodies_does_not_truncate_unpersisted`, `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`, `crates/iroha_core/src/kura.rs:8126`)
- Gaps:
  - no fault-injection coverage for append/index/fsync failure handling without panic
  - no performance regression test for large pending queues and enqueue budget-check cost
  - no long-history eviction latency test under lock contention

### Production WSV

- Existing coverage:
  - contention fallback behavior: `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - lock-order safety around tiered backend: `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- Gaps:
  - no quantitative contention test asserting max acceptable commit hold time under heavy world commits
  - no regression test for panic-free handling if DA cursor advancement invariants break unexpectedly

### IVM WSV Mock Host

- Existing coverage:
  - permission JSON parser semantics and peer parsing (`crates/ivm/src/mock_wsv.rs:5234`, `crates/ivm/src/mock_wsv.rs:5332`)
  - syscall smoke tests around TLV decode and JSON decode (`crates/ivm/src/mock_wsv.rs:5962`, `crates/ivm/src/mock_wsv.rs:6078`)
- Gaps:
  - no unauthorized-admin-alias rejection tests
  - no oversized TLV envelope rejection tests in `INPUT_PUBLISH_TLV`
  - no benchmark/guardrail tests around checkpoint/restore clone cost

## Prioritized Remediation Plan

### Phase 1 (High-impact hardening)

1. Replace Kura writer `panic!` branches with recoverable error propagation + degraded-health signaling.
- Target files: `crates/iroha_core/src/kura.rs`
- Acceptance:
  - injected append/index/fsync failures do not panic
  - errors are surfaced through telemetry/logging and writer remains controllable

2. Add bounded envelope checks for IVM mock-host TLV publish and JSON envelope paths.
- Target files: `crates/ivm/src/mock_wsv.rs`
- Acceptance:
  - oversized payloads are rejected before allocation-heavy processing
  - new tests cover both TLV and JSON oversized cases

3. Enforce explicit caller permission checks for JSON admin aliases (or gate aliases behind strict test-only feature flags and document clearly).
- Target files: `crates/ivm/src/mock_wsv.rs`
- Acceptance:
  - unauthorized caller cannot mutate role/permission/peer state through aliases

### Phase 2 (Hot-path performance)

4. Make Kura budget accounting incremental.
- Replace per-enqueue full pending-queue recomputation with maintained counters updated on enqueue/persist/drop.
- Acceptance:
  - enqueue cost near O(1) for pending-bytes calculation
  - regression benchmark shows stable latency as pending depth grows

5. Reduce eviction lock hold time.
- Options: segmented compaction, chunked copy with lock release boundaries, or background maintenance mode with bounded foreground blocking.
- Acceptance:
  - large-history eviction latency decreases and foreground operations remain responsive

6. Shorten coarse `view_lock` critical section where feasible.
- Evaluate splitting commit phases or snapshotting staged deltas to minimize exclusive hold windows.
- Acceptance:
  - contention metrics demonstrate reduced 99p hold time under heavy block commits

### Phase 3 (Operational guardrails)

7. Introduce bounded/coalesced wake signaling for Kura writer and sidecar queue backpressure/caps.
8. Expand telemetry dashboards for:
- `view_lock` wait/hold distributions
- eviction duration and reclaimed bytes per run
- budget-check enqueue latency

## Suggested Test Additions

1. `kura_writer_io_failures_do_not_panic` (unit, fault injection)
2. `kura_budget_check_scales_with_pending_depth` (performance regression)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (integration/perf)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (contention regression)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (resilience)
6. `mock_wsv_admin_alias_requires_permissions` (security regression)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (DoS guard)
8. `mock_wsv_checkpoint_restore_cost_regression` (perf benchmark)

## Notes on Scope and Confidence

- Findings for `crates/iroha_core/src/kura.rs` and `crates/iroha_core/src/state.rs` are production-path findings.
- Findings for `crates/ivm/src/mock_wsv.rs` are explicitly test/dev host scoped, per file-level documentation.
- No ABI versioning changes are required by this audit itself.
