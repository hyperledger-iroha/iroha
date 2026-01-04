# AMX Execution & Operations Guide

**Status:** Draft (NX-17)  
**Audience:** Core protocol, AMX/consensus engineers, SRE/Telemetry, SDK & Torii teams  
**Context:** Completes the roadmap item “Documentation (owner: Docs) — update `docs/amx.md` with timing diagrams, error catalog, operator expectations, and developer guidance for generating/using PVOs.”【roadmap.md:2497】

## Summary

Atomic cross-data-space transactions (AMX) let a single submission touch multiple data spaces (DS) while preserving 1 s slot finality, deterministic failure codes, and confidentiality for private DS fragments. This guide captures the timing model, canonical error handling, operator evidence requirements, and developer expectations for Proof Verification Objects (PVOs) so the roadmap deliverable remains self-contained outside the Nexus design paper (`docs/source/nexus.md`).

Key guarantees:

- Every AMX submission receives deterministic prepare/commit budgets; overruns abort with documented codes rather than hanging lanes.
- DA samples that miss the budget are logged as missing availability evidence and the transaction remains queued for the next slot instead of silently stalling throughput.
- Proof Verification Objects (PVOs) decouple heavy proofs from the 1 s slot by letting clients/batchers pre-register artefacts that the nexus host validates quickly in-slot.
- IVM hosts derive per-dataspace AXT policy from the Space Directory: handles must target the lane advertised in the catalog, present the latest manifest root, satisfy expiry_slot, handle_era, and sub_nonce minima, and reject unknown dataspaces with `PermissionDenied` before execution.
- Slot expiry uses `nexus.axt.slot_length_ms` (default `1` ms, validated between `1` ms and `600_000` ms) plus the bounded `nexus.axt.max_clock_skew_ms` (default `0` ms, capped by the slot length and `60_000` ms). Hosts compute `current_slot = block.creation_time_ms / slot_length_ms`, apply the skew allowance to proof and handle expiry checks, and reject handles that advertise a larger skew than the configured limit.
- Proof cache TTL bounds reuse: `nexus.axt.proof_cache_ttl_slots` (default `1`, validated `1`–`64`) limits how long accepted or rejected proofs stay in the host cache; entries drop once the TTL window or the proof’s `expiry_slot` elapses so replay protection stays bounded.
- Replay ledger retention: `nexus.axt.replay_retention_slots` (default `128`, validated `1`–`4_096`) caps how many slots of handle-usage history are retained for replay rejection across peers/restarts; align it with the longest handle-validity window you expect operators to issue. The ledger is persisted in WSV, hydrated on startup, and pruned deterministically once the retention window or handle expiry elapses so peer switches do not reopen replay gaps.
- Debugging cache status: Torii exposes `/v1/debug/axt/cache` (telemetry/developer gate) to return the current AXT policy snapshot version, the most recent reject (lane/reason/version), cached proofs (dataspace/status/manifest root/slots), and reject hints (`next_min_handle_era`/`next_min_sub_nonce`). Use this endpoint to confirm slot/manifest rotations are reflected in cache state and to refresh handles deterministically during troubleshooting.

## Slot Timing Model

### Timeline

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- Budgets align with the global ledger plan: mempool 70 ms, DA commit ≤300 ms, consensus 300 ms, IVM/AMX 250 ms, settlement 40 ms, guard 40 ms.【roadmap.md:2529】
- Transactions breaching the DA window are logged as missing availability evidence and retried in the next slot; all other breaches surface codes such as `AMX_TIMEOUT` or `SETTLEMENT_ROUTER_UNAVAILABLE`.
- The guard slice absorbs telemetry export and final auditing so the slot still closes at 1 s even if exporters lag briefly.
- Configuration tips: defaults keep expiry strict (`slot_length_ms = 1`, `max_clock_skew_ms = 0`). For a 1 s cadence set `slot_length_ms = 1_000` and `max_clock_skew_ms = 250`; for a 2 s cadence use `slot_length_ms = 2_000` and `max_clock_skew_ms = 500`. Values outside the validated window (`1`–`600_000` ms or `max_clock_skew_ms` greater than the slot length/`60_000` ms) are rejected at config-parse time, and advertised handle skew must stay within the configured bound.

### Cross-DS swim lane

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

Each DS fragment must finish its 30 ms prepare window before the lane assembles the slot. Missing proofs stay in the mempool for the next slot rather than blocking peers.

### Instrumentation checklist

| Metric / Trace | Source | SLO / Alert | Notes |
|----------------|--------|-------------|-------|
| `iroha_slot_duration_ms` (histogram) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | Ci gate described in `ans3.md`. |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (commit hook) | ≥0.95 per 30 min window | Derived from missing-availability telemetry so every block updates the gauge (`crates/iroha_core/src/telemetry.rs:3524`,`crates/iroha_core/src/telemetry.rs:4558`). |
| `iroha_amx_prepare_ms` | IVM host | p95 ≤ 30 ms per DS scope | Drives `AMX_TIMEOUT` aborts. |
| `iroha_amx_commit_ms` | IVM host | p95 ≤ 40 ms per DS scope | Covers delta merge + trigger execution. |
| `iroha_ivm_exec_ms` | IVM host | Alert if >250 ms per lane | Mirrors the IVM overlay chunk execution window. |
| `iroha_amx_abort_total{stage}` | Executor | Alert if >0.05 aborts/slot or sustained single-stage spikes | Stage labels: `prepare`, `exec`, `commit`. |
| `iroha_amx_lock_conflicts_total` | AMX scheduler | Alert if >0.1 conflicts/slot | Indicates inaccurate R/W sets. |
| `iroha_axt_policy_reject_total{lane,reason}` | IVM host | Watch for spikes | Distinguishes manifest/lane/era/sub_nonce/expiry rejects. |
| `iroha_axt_policy_snapshot_cache_events_total{event}` | IVM host | Expect cache_miss only on startup/manifest change | Sustained misses indicate stale policy hydration. |
| `iroha_axt_proof_cache_events_total{event}` | IVM host | Expect mostly `hit`/`miss` | `reject`/`expired` spikes usually indicate manifest drift or stale proofs. |
| `iroha_axt_proof_cache_state{dsid,status,manifest_root_hex,verified_slot}` | IVM host | Inspect cached proofs | Gauge value is expiry_slot (with skew applied) for the cached proof. |
| Missing availability evidence (`sumeragi_da_gate_block_total{reason="missing_availability_qc"}`) | Lane telemetry | Alert if >5% of tx per DS | Means attesters or proofs are lagging. |

`/v1/debug/axt/cache` mirrors the `iroha_axt_proof_cache_state` gauge with a per-dataspace snapshot (status, manifest root, verified/expiry slots) for operators.

`iroha_amx_commit_ms` and `iroha_ivm_exec_ms` share the same latency buckets as
`iroha_amx_prepare_ms`. The abort counter tags every rejection with the lane id
and stage (`prepare` = overlay build/validation, `exec` = IVM chunk execution,
`commit` = delta merge + trigger replay) so telemetry can highlight whether
contention comes from read/write mismatches or post-state merges.

Operators must archive these metrics for audit alongside slot acceptance evidence and note regressions in `status.md`.

### AXT golden fixtures

Norito fixtures for the descriptor/handle/policy snapshot live at `crates/iroha_data_model/tests/fixtures/axt_golden.rs`, with a regeneration helper in `crates/iroha_data_model/tests/axt_policy_vectors.rs` (`print_golden_vectors`). CoreHost consumes the same fixtures in `core_host_enforces_fixture_snapshot_fields` (`crates/ivm/tests/core_host_policy.rs`) to exercise lane binding, manifest root matching, expiry_slot freshness, handle_era/sub_nonce minima, and missing-dataspace rejections.
- A multi-dataspace JSON fixture (`crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json`) pins the descriptor/touch schema, canonical Norito bytes, and Poseidon binding (`compute_descriptor_binding`). The `axt_descriptor_fixture` test guards the encoded bytes, and SDKs can use `AxtDescriptorBuilder::builder` plus `TouchManifest::from_read_write` to assemble deterministic samples for docs/SDKs.

### Lane catalog mapping and manifests

- AXT policy snapshots are built from the Space Directory manifest set and lane catalog. Each dataspace is mapped to its configured lane; active manifests contribute the manifest hash, activation epoch (`min_handle_era`), and sub-nonce floor. UAID bindings without an active manifest still emit a policy entry with a zeroed manifest root so lane gating remains active until a real manifest lands.
- `current_slot` in the snapshot is derived from the committed block height, so handles with an `expiry_slot` lower than the current height are rejected even before a dedicated slot clock is plumbed through.
- Telemetry surfaces the hydrated snapshot as `iroha_axt_policy_snapshot_version` (lower 64 bits of the Norito-encoded snapshot hash) and cache events via `iroha_axt_policy_snapshot_cache_events_total{event=cache_hit|cache_miss}`. Reject counters use the labels `lane`, `manifest`, `era`, `sub_nonce`, and `expiry` so operators can immediately see which field blocked a handle.

### Cross-dataspace composability checklist

- Confirm every dataspace listed in the Space Directory has a lane entry and an active manifest; rotation should refresh bindings and manifest roots before issuing new handles. Zeroed roots mean handles will stay denied until manifests are present, and hosts/block validation now reject handles that present zeroed manifest roots.
- On startup and after Space Directory changes, expect one `cache_miss` followed by steady `cache_hit` events on the policy snapshot metric; a sustained miss rate points to a stale or missing manifest feed.
- When a handle is rejected, look at `iroha_axt_policy_reject_total{lane,reason}` and the snapshot version to decide whether to request a refreshed handle (`expiry`/`era`/`sub_nonce`) or to repair the lane/manifest binding (`lane`/`manifest`). The Torii debug endpoint `/v1/debug/axt/cache` also returns `reject_hints` with `dataspace`, `target_lane`, `next_min_handle_era`, and `next_min_sub_nonce` so operators can refresh handles deterministically after a policy bump.

### SDK sample: remote spend without token egress

1. Build an AXT descriptor listing the dataspace that owns the asset plus any read/write touches required locally; keep the descriptor deterministic so the binding hash stays stable.
2. Call `AXT_TOUCH` for the remote dataspace with the manifest view you expect; optionally attach a proof via `AXT_VERIFY_DS_PROOF` if the host requires it.
3. Request or refresh the asset handle and invoke `AXT_USE_ASSET_HANDLE` with a `RemoteSpendIntent` that spends inside the remote dataspace (no bridge leg). Budget enforcement uses the handle’s `remaining`, `per_use`, `sub_nonce`, `handle_era`, and `expiry_slot` against the snapshot described above.
4. Commit via `AXT_COMMIT`; if the host returns `PermissionDenied`, use the reject label to decide whether to fetch a fresh handle (expiry/sub_nonce/era) or fix the manifest/lane binding.

## Operator Expectations

1. **Pre-slot readiness**
   - Ensure DA attester pools per profile (A=12, B=9, C=7) are healthy; attester churn is recorded in the Space Directory snapshot for the slot.
   - Validate `iroha_amx_prepare_ms` is below budget on representative runners before enabling new workload mixes.

2. **In-slot monitoring**
   - Alert on missing-availability spikes (>5% for two consecutive slots) and on `AMX_TIMEOUT` because both indicate missed budgets.
   - Track PVO cache utilisation (`iroha_pvo_cache_hit_ratio`, exported by the proof service) to prove that off-path verification keeps up with submissions.

3. **Evidence capture**
   - Attach DA receipt sets, AMX prepare histograms, and PVO cache reports to the nightly artefact bundle referenced from `status.md`.
   - Record chaos drill outputs in `ops/drill-log.md` whenever DA jitter, oracle stalls, or buffer depletion tests run.

4. **Runbook maintenance**
   - Update the Android/Swift SDK runbooks whenever AMX error codes or overrides change so client teams inherit the deterministic failure semantics.
   - Keep configuration snippets (e.g., `iroha_config.amx.*`) in sync with the canonical parameters in `docs/source/nexus.md`.

## Telemetry & Troubleshooting

### Telemetry quick reference

| Source | What to capture | Command / Path | Evidence expectations |
|--------|-----------------|----------------|-----------------------|
| Prometheus (`iroha_telemetry`) | Slot and AMX SLOs: `iroha_slot_duration_ms`, `iroha_amx_prepare_ms`, `iroha_amx_commit_ms`, `iroha_da_quorum_ratio`, `iroha_amx_abort_total{stage}` | Scrape `https://$TORII/telemetry/metrics` or export from the dashboards described in `docs/source/telemetry.md`. | Attach histogram snapshots (and alert history if triggered) to the nightly `status.md` note so auditors can see p95/p99 values and alert states. |
| Proof service metrics | PVO cache health: `iroha_pvo_cache_hit_ratio`, cache fill/evict counters, proof queue depth | `GET /metrics` on the proof service (`IROHA_PVO_METRICS_URL`) or via the shared OTLP collector. | Export the cache hit ratio and queue depth alongside the AMX slot metrics so the roadmap OA/PVO gate has deterministic artefacts. |
| Acceptance harness | End-to-end mixed load (slot/DA/RBC/PVO) under controlled jitter | Re-run `ci/acceptance/slot_1s.yml` (or the same job in CI) and archive the log bundle + generated artefacts in `artifacts/acceptance/slot_1s/<timestamp>/`. | Required before GA and whenever pacemaker/DA settings change; include the YAML run summary plus the Prometheus snapshots in the operator hand-off packet. |

### Troubleshooting playbook

| Symptom | Inspect first | Recommended remediation |
|---------|---------------|--------------------------|
| `iroha_slot_duration_ms` p95 creeps above 1 000 ms | Prometheus export from `/telemetry/metrics` plus the latest `/v1/sumeragi/rbc` snapshot to confirm DA deferrals; compare against the last `ci/acceptance/slot_1s.yml` artefact. | Lower AMX batch sizes or enable additional RBC collectors (`sumeragi.npos.k_aggregators`), then rerun the acceptance harness and capture the new telemetry evidence. |
| Missing availability spike | `/v1/sumeragi/rbc/sessions` backlog fields (`lane_backlog`, `dataspace_backlog`) alongside attester health dashboards. | Remove unhealthy attesters, temporarily increase `redundant_send_r` to speed up delivery, and publish the remediation notes in `status.md`. Attach updated RBC snapshots once the backlog clears. |
| Frequent `PVO_MISSING_OR_EXPIRED` in receipts | Proof service cache metrics + the issuer’s PVO scheduler logs. | Regenerate stale PVO artefacts, shorten the rotation cadence, and ensure every SDK refreshes the handle before `expiry_slot`. Include the proof-service metrics in the evidence bundle to prove the cache recovered. |
| Repeated `AMX_LOCK_CONFLICT` or `AMX_TIMEOUT` | `iroha_amx_lock_conflicts_total`, `iroha_amx_prepare_ms`, and the affected transaction manifests. | Re-run the Norito static analyzer, correct the read/write selectors (or split the batch), and publish the updated manifest fixtures so the conflict counter returns to baseline. |
| `SETTLEMENT_ROUTER_UNAVAILABLE` alerts | Settlement router logs (`docs/settlement-router.md`), treasury buffer dashboards, and the affected receipts. | Top up XOR buffers or flip the lane to XOR-only mode, document the treasury action, and rerun the slot acceptance test to prove settlement resumed. |

### AXT rejection signals

- Reason codes are captured as `AxtRejectReason` (`lane`, `manifest`, `era`, `sub_nonce`, `expiry`, `missing_policy`, `policy_denied`, `proof`, `budget`, `replay_cache`, `descriptor`, `duplicate`). Block validation now surfaces `AxtEnvelopeValidationFailed { message, reason, snapshot_version }`, so incidents can pin the rejection to a specific policy snapshot.
- `/v1/debug/axt/cache` returns `{ policy_snapshot_version, last_reject, cache, hints }`, where `last_reject` carries the lane/reason/version of the most recent host rejection and `hints` provide `next_min_handle_era`/`next_min_sub_nonce` refresh guidance alongside the cached proof state.
- Alert template: page when `iroha_axt_policy_reject_total{reason="manifest"}` or `{reason="expiry"}` spikes over a 5‑minute window, attach the `last_reject` snapshot + `policy_snapshot_version` from the Torii debug endpoint to the incident, and use the hint payload to request refreshed handles before retrying.

## Proof Verification Objects (PVOs)

### Structure

PVOs are Norito-encoded envelopes that let clients prove heavy work ahead of time. The canonical fields are:

| Field | Description |
|-------|-------------|
| `circuit_id` | Static identifier for the proof system/statement (e.g., `amx.transfer.v1`). |
| `vk_hash` | Blake2b-256 hash of the verifying key referenced by the DS manifest. |
| `proof_digest` | Poseidon digest of the serialized proof payload stored in the off-slot PVO registry. |
| `max_k` | Upper bound on the AIR domain; hosts reject proofs exceeding the advertised size. |
| `expiry_slot` | Slot height after which the artefact is invalid; keeps stale proofs out of lanes. |
| `profile` | Optional hint (e.g., DS profile A/B/C) to help schedulers batch compatible proofs. |

The Norito schema lives beside the data model definitions in `crates/iroha_data_model/src/nexus` so SDKs can derive it without serde.

### Generation pipeline

1. **Compile circuit metadata** — Export `circuit_id`, verifying key, and max trace size from your prover build (usually via `fastpq_prover` reports).
2. **Produce proof artefacts** — Run the out-of-slot prover and store full transcripts plus commitments.
3. **Register via the proof service** — Submit the Norito PVO payload to the off-slot verifier (see roadmap NX-17 proof pipeline). The service verifies once, pins the digest, and exposes the handle via Torii.
4. **Reference in transactions** — Attach the PVO handle to AMX builders (`amx_touch` or higher-level SDK helpers). Hosts look up the digest, verify the cached result, and only recompute inside the slot if the cache is cold.
5. **Rotate on expiry** — SDKs must refresh any cached handle before `expiry_slot`. Expired objects trigger `PVO_MISSING_OR_EXPIRED`.

### Developer checklist

- Declare read/write sets accurately so AMX can prefetch locks and avoid `AMX_LOCK_CONFLICT`.
- Bundle deterministic allowance proofs in the same UAID manifest update when cross-DS transfers touch regulated DSes.
- Retrying strategy: missing availability evidence → no action (the tx stays in mempool); `AMX_TIMEOUT` or `PVO_MISSING_OR_EXPIRED` → rebuild artefacts and back off exponentially.
- Tests should include both cache hits and cold starts (forcing the host to verify the proof with the same `max_k`) to guard against determinism regressions.
- Proof blobs (`ProofBlob`) MUST encode an `AxtProofEnvelope { dsid, manifest_root, da_commitment?, proof }`; hosts bind proofs to the Space Directory manifest root and cache pass/fail results per dataspace/slot with `iroha_axt_proof_cache_events_total{event="hit|miss|expired|reject|cleared"}`. Expired or manifest-mismatched artefacts are rejected before commit and subsequent retries in the same slot short-circuit on the cached `reject`.
- Proof cache reuse is slot-scoped: verified proofs stay hot across envelopes within the same slot and are evicted automatically when the slot advances so retries remain deterministic.

### Static read/write analyzer

Compile-time selectors must match the contract’s actual behaviour before AMX can
prefetch locks or apply UAID manifests. The new `ivm::analysis` module
(`crates/ivm/src/analysis.rs`) exposes `analyze_program(&[u8])` which decodes a
`.to` artefact, tallies register reads/writes, memory ops, and syscall usage,
and produces a JSON-friendly report that the SDK manifests can embed. Run it
alongside `koto_lint` when publishing UAIDs so the generated R/W summary is
captured in the evidence bundle referenced during NX-17 readiness reviews.

## Space Directory policy enforcement

AXT handle verification now defaults to the Space Directory snapshot when the host has access to it (CoreHost in tests, WsvHost in integration flows). Per-dataspace policy entries carry `manifest_root`, `target_lane`, `min_handle_era`, `min_sub_nonce`, and `current_slot`. Hosts enforce:

- lane binding: handle `target_lane` must match the Space Directory entry;
- manifest binding: non-zero `manifest_root` values must match the handle’s `manifest_view_root`;
- expiry: `current_slot` greater than the handle’s `expiry_slot` is rejected;
- counters: `handle_era` and `sub_nonce` must be at least the advertised minima;
- membership: handles for dataspaces absent from the snapshot are denied.

Failures map to `PermissionDenied`, and the CoreHost policy snapshot tests in `crates/ivm/tests/core_host_policy.rs` cover allow/deny cases for each field.
Block validation also requires non-empty proofs per dataspace with `expiry_slot` covering both the policy/block slot and the handle’s expiry, aggregates handle budgets per dataspace, and advances `min_handle_era`/`min_sub_nonce` as envelopes commit so replayed sub-nonces are rejected even after Space Directory rebuilds.

## Error Catalog

Canonical codes live in `crates/iroha_data_model/src/errors.rs`. Operators must surface them verbatim in metrics/logs, and SDKs should map them to actionable retries.

| Code | Trigger | Operator response | SDK guidance |
|------|---------|-------------------|--------------|
| Missing availability evidence (telemetry) | Fewer than `q` attester receipts verified before 300 ms. | Inspect attester health, widen sampling parameters for next slot, keep the transaction queued, and capture the missing-availability counters for runbook evidence. | No action; retry happens automatically because the tx stays enqueued. |
| `DA_DEADLINE_EXCEEDED` | Δ window elapsed without meeting DA quorum. | Resign offending attesters, publish incident note, force clients to resubmit. | Rebuild transaction once attesters are back; consider splitting the batch. |
| `AMX_TIMEOUT` | Combined prepare/commit exceeded 250 ms per DS slice. | Capture flamegraphs, verify R/W sets, and compare against `iroha_amx_prepare_ms`. | Retry with smaller batch or after reducing contention. |
| `AMX_LOCK_CONFLICT` | Host detected overlapping write sets or unsignaled touches. | Inspect UAID manifests and static analysis reports; update manifests if missing selectors. | Recompile transaction with corrected read/write declarations. |
| `PVO_MISSING_OR_EXPIRED` | Referenced PVO handle not in cache or past `expiry_slot`. | Check proof service backlog, regenerate artefact, and verify Torii indexes. | Refresh proof artefact and resubmit with the new handle. |
| `RWSET_UNBOUNDED` | Static analysis could not bound a read/write selector. | Reject deployment, log selector stack trace, require developer fix before retry. | Update contract to emit explicit selectors. |
| `HEAVY_INSTRUCTION_DISALLOWED` | Contract invoked an instruction banned from AMX lanes (e.g., large FFT without PVO). | Make sure Norito builder uses the approved opcode set before re-enabling. | Split workload or add a pre-computed proof. |
| `SETTLEMENT_ROUTER_UNAVAILABLE` | Router could not compute deterministic conversion (missing path, buffer drained). | Engage Treasury to refill buffers or flip XOR-only mode; record in settlement runbook. | Retry after buffer alert clears; show user-facing warning. |

SDK teams should mirror these codes in integration tests so `iroha_cli`, Android, Swift, JS, and Python surfaces agree on error text and recommended actions.

### AXT rejection observability

- Torii surfaces policy failures as `ValidationFail::AxtReject` (and block validation as `AxtEnvelopeValidationFailed`) with a stable reason label, the active `snapshot_version`, optional `lane`/`dataspace` identifiers, and hint fields for `next_min_handle_era`/`next_min_sub_nonce`. SDKs should bubble these fields to users so stale handles can be refreshed deterministically.
- Torii now also stamps HTTP responses with `X-Iroha-Axt-*` headers for quick triage: `Code`/`Reason`, `Snapshot-Version`, `Dataspace`, `Lane`, and optional `Next-Handle-Era`/`Next-Sub-Nonce`. ISO bridge rejections carry matching `PRTRY:AXT_*` reason codes and the same detail strings so dashboards and operators can key alerts off the AXT failure class without decoding the full payload.
- Hosts log `AXT policy rejection recorded` with the same fields and export them via telemetry: `iroha_axt_policy_reject_total{lane,reason}` counts rejects, and `iroha_axt_policy_snapshot_version` tracks the hash of the active snapshot. Proof cache state remains available via `/v1/debug/axt/cache` (dataspace/status/manifest root/slots).
- Alerting: watch for spikes in `iroha_axt_policy_reject_total` grouped by `reason` and page with the `snapshot_version` from logs/ValidationFail to confirm whether operators need to rotate manifests (lane/manifest rejects) or refresh handles (era/sub_nonce/expiry). Pair alerts with the proof-cache endpoint to confirm whether rejects are cache-related or policy-related.

## Testing & Evidence

- CI must run the `ci/acceptance/slot_1s.yml` suite (30 min mixed workloads) and fail merges when the slot/DA/telemetry thresholds are not met, as spelled out in `ans3.md`.
- Chaos drills (attester jitter, oracle stalls, buffer depletion) must be executed at least quarterly with artefacts archived under `ops/drill-log.md`.
- Status updates should include: most recent slot SLO report, outstanding error spikes, and a link to the latest PVO cache snapshot so roadmap stakeholders can audit readiness.

By following this guide, contributors satisfy the roadmap requirement for AMX documentation and give operators and developers a single reference for timing, telemetry, and PVO workflows.
