# AMX Execution & Operations Guide

**Status:** Current source-adjacent protocol and operations reference  
**Audience:** Core protocol, AMX/consensus engineers, SRE/Telemetry, SDK & Torii teams

## Summary

Atomic cross-data-space transactions (AMX) let a single submission touch
multiple data spaces (DS) while preserving deterministic failure codes and
confidentiality for private DS fragments. This reference records the
implemented timing controls, AXT proof and handle formats, error handling, and
operator evidence requirements alongside the Nexus design (`nexus.md`).

Key guarantees:

- Every AMX submission receives deterministic prepare/commit budgets; overruns abort with documented codes rather than hanging lanes.
- DA samples that miss the budget are logged as missing availability evidence and the transaction remains queued for the next slot instead of silently stalling throughput.
- AXT proof envelopes bind proof bytes to a dataspace, its active manifest root,
  and the FastPQ V1 verifier metadata before the host caches the verification
  result for the configured slot window.
- IVM hosts derive per-dataspace AXT policy and issuer identity from committed Space Directory state. A handle must carry a valid domain-separated V1 signature from the single-key UAID account bound to the active `(dataspace, manifest root)`, target the catalog lane, use the exact active era and exact next counter, and satisfy expiry. Missing, ambiguous, multisignature, or inconsistent issuer indexes fail closed before FASTPQ verification.
- Slot expiry uses `nexus.axt.slot_length_ms` (default `1` ms, validated between `1` ms and `600_000` ms) plus the bounded `nexus.axt.max_clock_skew_ms` (default `0` ms, capped by the slot length and `60_000` ms). Hosts compute `current_slot = block.creation_time_ms / slot_length_ms`, apply the skew allowance to proof and handle expiry checks, and reject handles that advertise a larger skew than the configured limit.
- Proof cache TTL bounds reuse: `nexus.axt.proof_cache_ttl_slots` (default `1`, validated `1`–`64`) limits how long accepted or rejected proofs stay in the host cache; entries drop once the TTL window or the proof’s `expiry_slot` elapses so replay protection stays bounded.
- Replay ledger retention: `nexus.axt.replay_retention_slots` (default `128`, validated `1`–`4_096`) sets the minimum slot window of handle-usage history retained for replay rejection across peers/restarts; align it with the longest handle-validity window you expect operators to issue. The ledger is persisted in WSV, hydrated on startup, and pruned deterministically once both the retention window and handle expiry have elapsed (whichever is later). A block carries the deterministic post-state policy snapshot; Kura replay installs that snapshot and rebuilds the ledger without advancing counters a second time.
- Debugging cache status: Torii exposes `/v1/debug/axt/cache` (telemetry/developer gate) to return the current AXT policy snapshot version, the most recent reject (lane/reason/version), cached proofs (dataspace/status/manifest root/slots), and reject hints (`active_handle_era`/`next_handle_counter`). Use this endpoint to confirm slot/manifest rotations are reflected in cache state and to refresh handles deterministically during troubleshooting.

## Slot Timing Model

### Timeline

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- The operational budget is mempool 70 ms, DA commit ≤300 ms, consensus 300 ms,
  IVM/AMX 250 ms, settlement 40 ms, and a 40 ms guard.
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
  │           │               │     │             │       │ verify proof/DA │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

Each DS fragment must finish its 30 ms prepare window before the lane assembles the slot. Missing proofs stay in the mempool for the next slot rather than blocking peers.

### Instrumentation checklist

| Metric / Trace | Source | SLO / Alert | Notes |
|----------------|--------|-------------|-------|
| `iroha_slot_duration_ms` (histogram) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | Capture with the AXT acceptance evidence. |
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
| Missing availability evidence (`sumeragi_da_gate_block_total{reason="missing_local_data"}`) | Lane telemetry | Alert if >5% of tx per DS | Means attesters or proofs are lagging. |

`/v1/debug/axt/cache` mirrors the `iroha_axt_proof_cache_state` gauge with a per-dataspace snapshot (status, manifest root, verified/expiry slots) for operators.

`iroha_amx_commit_ms` and `iroha_ivm_exec_ms` share the same latency buckets as
`iroha_amx_prepare_ms`. The abort counter tags every rejection with the lane id
and stage (`prepare` = overlay build/validation, `exec` = IVM chunk execution,
`commit` = delta merge + trigger replay) so telemetry can highlight whether
contention comes from read/write mismatches or post-state merges.

Operators archive these metrics with the corresponding slot acceptance
evidence.

### AXT golden fixtures

Norito fixtures for the descriptor/handle/policy snapshot live at `crates/iroha_data_model/tests/fixtures/axt_golden.rs`, with a regeneration helper in `crates/iroha_data_model/tests/axt_policy_vectors.rs` (`print_golden_vectors`). CoreHost consumes the same fixtures in `core_host_enforces_fixture_snapshot_fields` (`crates/ivm/tests/core_host_policy.rs`) to exercise lane binding, manifest root matching, expiry freshness, exact era/sub-nonce matching, and missing-dataspace rejections.
- A multi-dataspace JSON fixture (`crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json`) pins the descriptor/touch schema, canonical header-framed Norito bytes for the data-model type, and the Poseidon binding derived from the bare Norito payload (`compute_descriptor_binding`). Poseidon byte packing appends `0x01` and zero-pads to an eight-byte boundary before field-sponge padding. The `axt_descriptor_fixture` test guards the encoded bytes, and SDKs can use `AxtDescriptorBuilder::builder` plus `TouchManifest::from_read_write` to assemble deterministic samples for docs/SDKs.

### Lane catalog mapping and manifests

- AXT policy snapshots are built from the Space Directory manifest set and lane catalog. Each dataspace is mapped to its configured lane; active manifests contribute the manifest hash, exact activation epoch (`active_handle_era`), and exact next counter (`next_handle_counter`). UAID bindings without an active manifest still emit a policy entry with a zeroed manifest root so lane gating remains active until a real manifest lands.
- `current_slot` in the snapshot is derived from the latest committed block timestamp (`creation_time_ms / slot_length_ms`), falling back to the block height only before a committed header is available.
- Telemetry surfaces the hydrated snapshot as `iroha_axt_policy_snapshot_version` (lower 64 bits of the Norito-encoded snapshot hash) and cache events via `iroha_axt_policy_snapshot_cache_events_total{event=cache_hit|cache_miss}`. Reject counters use the labels `lane`, `manifest`, `era`, `sub_nonce`, and `expiry` so operators can immediately see which field blocked a handle.

### Cross-dataspace composability checklist

- Confirm every dataspace listed in the Space Directory has a lane entry and an active manifest; rotation should refresh bindings and manifest roots before issuing new handles. Zeroed roots mean handles will stay denied until manifests are present, and hosts/block validation now reject handles that present zeroed manifest roots.
- On startup and after Space Directory changes, expect one `cache_miss` followed by steady `cache_hit` events on the policy snapshot metric; a sustained miss rate points to a stale or missing manifest feed.
- When a handle is rejected, look at `iroha_axt_policy_reject_total{lane,reason}` and the snapshot version to decide whether to request a refreshed handle (`expiry`/`era`/`sub_nonce`) or to repair the lane/manifest binding (`lane`/`manifest`). The Torii debug endpoint `/v1/debug/axt/cache` also returns `reject_hints` with `dataspace`, `target_lane`, `active_handle_era`, and `next_handle_counter` so operators can refresh handles deterministically after a policy bump.

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
   - Track `iroha_axt_proof_cache_events_total{event}` and
     `iroha_axt_proof_cache_state` to verify cache hit, expiry, rejection, and
     eviction behaviour.

3. **Evidence capture**
   - Attach DA receipt sets, AMX prepare histograms, and AXT proof-cache
     snapshots to the run artefact bundle.
   - Record chaos drill outputs in `ops/drill-log.md` whenever DA jitter, oracle stalls, or buffer depletion tests run.

4. **Runbook maintenance**
   - Update the Android/Swift SDK runbooks whenever AMX error codes or overrides change so client teams inherit the deterministic failure semantics.
   - Keep configuration snippets in sync with the canonical `nexus.axt`
     parameters documented in `references/configuration.md`.

## Telemetry & Troubleshooting

### Telemetry quick reference

| Source | What to capture | Command / Path | Evidence expectations |
|--------|-----------------|----------------|-----------------------|
| Prometheus (`iroha_telemetry`) | Slot and AMX SLOs: `iroha_slot_duration_ms`, `iroha_amx_prepare_ms`, `iroha_amx_commit_ms`, `iroha_da_quorum_ratio`, `iroha_amx_abort_total{stage}` | Scrape `https://$TORII/telemetry/metrics` or export from the dashboards described in `telemetry.md`. | Attach histogram snapshots and alert history to the run bundle so auditors can see p95/p99 values and alert states. |
| Torii Sumeragi telemetry | Aggregated collector activity, missing-chunk totals, bounded pre-session queues, and DA availability counters (`sumeragi_da_gate_block_total{reason="missing_local_data"}`, `sumeragi_rbc_da_reschedule_total`). | `GET /v1/sumeragi/telemetry`; capture `availability.collectors`, `rbc_backlog`, and `rbc_pending`. | Store timestamped JSON with the incident bundle. The payload is aggregate evidence and does not identify individual RBC sessions or publish a collector plan. |
| AXT proof cache | `iroha_axt_proof_cache_events_total{event}` and `iroha_axt_proof_cache_state{dsid,status,manifest_root_hex,verified_slot}` | Scrape the Torii metrics endpoint and inspect `GET /v1/debug/axt/cache` when the telemetry/developer gate is enabled. | Capture the policy snapshot version, cache state, and last rejection without retaining proof payloads. |

### Troubleshooting playbook

| Symptom | Inspect first | Recommended remediation |
|---------|---------------|--------------------------|
| `iroha_slot_duration_ms` p95 creeps above 1 000 ms | Prometheus export from `/telemetry/metrics` plus `GET /v1/sumeragi/telemetry`; compare `rbc_backlog` and `rbc_pending` with the preceding accepted run. | Lower AMX batch sizes or adjust the configured collector topology, then repeat the acceptance workload and capture the new telemetry evidence. |
| Missing availability spike | Aggregated `rbc_backlog` and `rbc_pending` fields, `availability.collectors`, status-store evictions, consensus logs, and attester health dashboards. | Repair the unhealthy attester or collector path and attach updated aggregate telemetry once the backlog clears. |
| Frequent `PVO_MISSING_OR_EXPIRED` in receipts | AXT proof-cache state, policy snapshot version, and the issuer’s proof/handle generation logs. | Regenerate the expired proof or handle and ensure the client refreshes it before `expiry_slot`. |
| Repeated `AMX_LOCK_CONFLICT` or `AMX_TIMEOUT` | `iroha_amx_lock_conflicts_total`, `iroha_amx_prepare_ms`, and the affected transaction manifests. | Re-run the Norito static analyzer, correct the read/write selectors (or split the batch), and publish the updated manifest fixtures so the conflict counter returns to baseline. |
| `SETTLEMENT_ROUTER_UNAVAILABLE` alerts | Settlement router logs (`../docs/settlement-router.md`), treasury buffer dashboards, and the affected receipts. | Top up XOR buffers or flip the lane to XOR-only mode, document the treasury action, and rerun the slot acceptance test to prove settlement resumed. |

### AXT rejection signals

- Reason codes are captured as `AxtRejectReason` (`lane`, `manifest`, `era`, `sub_nonce`, `expiry`, `missing_policy`, `policy_denied`, `proof`, `budget`, `replay_cache`, `descriptor`, `duplicate`). Block validation now surfaces `AxtEnvelopeValidationFailed { message, reason, snapshot_version }`, so incidents can pin the rejection to a specific policy snapshot.
- `/v1/debug/axt/cache` returns `{ policy_snapshot_version, last_reject, cache, hints }`, where `last_reject` carries the lane/reason/version of the most recent host rejection and `hints` provide exact `active_handle_era`/`next_handle_counter` guidance alongside the cached proof state.
- Alert template: page when `iroha_axt_policy_reject_total{reason="manifest"}` or `{reason="expiry"}` spikes over a 5‑minute window, attach the `last_reject` snapshot + `policy_snapshot_version` from the Torii debug endpoint to the incident, and use the hint payload to request refreshed handles before retrying.

## AXT Proof Envelopes

### Structure

The canonical data-model types live in
`crates/iroha_data_model/src/nexus/axt.rs`:

| Type / field | Contract |
|--------------|----------|
| `ProofBlob.payload` | Canonical Norito bytes for an `AxtProofEnvelope`; empty payloads are rejected. |
| `ProofBlob.expiry_slot` | Optional, non-zero proof expiry enforced against the current policy slot. |
| `AxtProofEnvelope.dsid` | Dataspace whose policy validates the proof. |
| `AxtProofEnvelope.manifest_root` | Non-zero root that must equal the active Space Directory policy root. |
| `AxtProofEnvelope.da_commitment` | Optional DA commitment bound into the envelope. |
| `AxtProofEnvelope.proof` | Non-empty backend proof bytes. |
| `AxtProofEnvelope.fastpq_binding` | Required FastPQ V1 source, claim, witness, policy, effect, verifier, and target-dataspace binding. |
| `committed_amount` | Optional non-zero scalar that must exactly match the canonical 16-byte little-endian `u128` in `axt_fastpq_committed_amount_v1` metadata inserted before the FastPQ batch seal and proof are generated. Missing or mismatched proof-bound metadata is rejected. |
| `amount_commitment` | Optional deterministic hidden-amount copy checked against the spend intent; recomputing it cannot replace or alter the proof-bound `committed_amount`. |

Issuers construct an unsigned `AssetHandleDraft`; signing consumes that draft
and returns an admission-ready `AssetHandle` with a mandatory signature. The
signature binds the exact genesis-derived `NetworkId`, committed issuer UAID and
manifest root, executing code root, ABI version/hash, dataspace, descriptor
digest, scope, subject, budget, group/lane context, exact active era/next
counter, expiry, and skew allowance. The host reconstructs this context from
committed state and the executing IVM before any FASTPQ work.

### Generation and use

1. Build and validate an `AxtDescriptor`, then derive its canonical Poseidon
   binding.
2. Produce an `AxtProofEnvelope` for each required dataspace with the active
   manifest root and a concrete FastPQ V1 binding.
3. Canonically encode the envelope into `ProofBlob.payload` and set a non-zero
   expiry when the prover needs bounded validity.
4. Attach the proof and any `AssetHandle` to the AXT envelope. The host verifies
   policy, manifest, lane, descriptor, amount, budget, and freshness bindings
   before commit.
5. Refresh expired proof blobs or handles. The canonical error catalog retains
   `PVO_MISSING_OR_EXPIRED` as the machine-readable missing/expiry code.

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
alongside `koto check` when publishing UAIDs so the generated R/W summary is
captured in the validation evidence bundle.

## Space Directory policy enforcement

AXT handle verification now defaults to the Space Directory snapshot when the host has access to it (CoreHost in tests, WsvHost in integration flows). Per-dataspace policy entries carry `manifest_root`, `target_lane`, `active_handle_era`, `next_handle_counter`, and `current_slot`. Hosts enforce:

- lane binding: handle `target_lane` must match the Space Directory entry;
- manifest binding: non-zero `manifest_root` values must match the handle’s `manifest_view_root`;
- expiry: `current_slot` greater than the handle’s `expiry_slot` is rejected;
- counters: `handle_era` must equal the active manifest era and `sub_nonce` must equal the next committed counter; stale and caller-selected future values are rejected, and advancement uses checked arithmetic;
- issuer authentication: the signature must bind every V1 policy/network field and verify with the single-key account resolved from the active manifest's committed UAID and dataspace binding;
- membership: handles for dataspaces absent from the snapshot are denied.

Failures map to `PermissionDenied`, and the CoreHost policy snapshot tests in `crates/ivm/tests/core_host_policy.rs` cover allow/deny cases for each field.
Block validation authenticates unique handles before FASTPQ verification, groups them by their exact V1 issuer/network/policy scope, and reconstructs the committed pre-state counter with checked subtraction from the advertised post-state. It then requires exact ordered counter progression and exact equality with the advertised post-state, with a consensus ceiling of 65,536 authenticated handles per block. It also requires non-empty proofs per dataspace with `expiry_slot` covering the policy slot (with the configured skew allowance) and not expiring before the handle, enforces descriptor binding plus touch manifests for declared specs (and rejects out-of-prefix entries), checks handle intent invariants, and aggregates handle budgets per dataspace.

## Error Catalog

Canonical codes live in `crates/iroha_data_model/src/errors.rs`. Operators must surface them verbatim in metrics/logs, and SDKs should map them to actionable retries.

| Code | Trigger | Operator response | SDK guidance |
|------|---------|-------------------|--------------|
| Missing availability evidence (telemetry) | Fewer than `q` attester receipts verified before 300 ms. | Inspect attester health, widen sampling parameters for next slot, keep the transaction queued, and capture the missing-availability counters for runbook evidence. | No action; retry happens automatically because the tx stays enqueued. |
| `DA_DEADLINE_EXCEEDED` | Δ window elapsed without meeting DA quorum. | Resign offending attesters, publish incident note, force clients to resubmit. | Rebuild transaction once attesters are back; consider splitting the batch. |
| `AMX_TIMEOUT` | Combined prepare/commit exceeded 250 ms per DS slice. | Capture flamegraphs, verify R/W sets, and compare against `iroha_amx_prepare_ms`. | Retry with smaller batch or after reducing contention. |
| `AMX_LOCK_CONFLICT` | Host detected overlapping write sets or unsignaled touches. | Inspect UAID manifests and static analysis reports; update manifests if missing selectors. | Recompile transaction with corrected read/write declarations. |
| `PVO_MISSING_OR_EXPIRED` | Referenced proof or handle is unavailable or past `expiry_slot`. | Inspect the AXT cache snapshot and regenerate the artefact. | Refresh the proof or handle and resubmit. |
| `RWSET_UNBOUNDED` | Static analysis could not bound a read/write selector. | Reject deployment, log selector stack trace, require developer fix before retry. | Update contract to emit explicit selectors. |
| `HEAVY_INSTRUCTION_DISALLOWED` | Contract invoked an instruction banned from AMX lanes (e.g., large FFT without PVO). | Make sure Norito builder uses the approved opcode set before re-enabling. | Split workload or add a pre-computed proof. |
| `SETTLEMENT_ROUTER_UNAVAILABLE` | Router could not compute deterministic conversion (missing path, buffer drained). | Engage Treasury to refill buffers or flip XOR-only mode; record in settlement runbook. | Retry after buffer alert clears; show user-facing warning. |

SDK teams should mirror these codes in integration tests so `iroha_cli`, Android, Swift, JS, and Python surfaces agree on error text and recommended actions.

### AXT rejection observability

- Torii surfaces policy failures as `ValidationFail::AxtReject` (and block validation as `AxtEnvelopeValidationFailed`) with a stable reason label, the active `snapshot_version`, optional `lane`/`dataspace` identifiers, and exact hint fields for `active_handle_era`/`next_handle_counter`. SDKs should bubble these fields to users so stale handles can be refreshed deterministically.
- Torii now also stamps HTTP responses with `X-Iroha-Axt-*` headers for quick triage: `Code`/`Reason`, `Snapshot-Version`, `Dataspace`, `Lane`, and optional `Active-Handle-Era`/`Next-Handle-Counter`. ISO bridge rejections carry matching `PRTRY:AXT_*` reason codes and the same detail strings so dashboards and operators can key alerts off the AXT failure class without decoding the full payload.
- Hosts log `AXT policy rejection recorded` with the same fields and export them via telemetry: `iroha_axt_policy_reject_total{lane,reason}` counts rejects, and `iroha_axt_policy_snapshot_version` tracks the hash of the active snapshot. Proof cache state remains available via `/v1/debug/axt/cache` (dataspace/status/manifest root/slots).
- Alerting: watch for spikes in `iroha_axt_policy_reject_total` grouped by `reason` and page with the `snapshot_version` from logs/ValidationFail to confirm whether operators need to rotate manifests (lane/manifest rejects) or refresh handles (era/sub_nonce/expiry). Pair alerts with the proof-cache endpoint to confirm whether rejects are cache-related or policy-related.

## Testing & Evidence

- Run the focused data-model, IVM, core-host, and integration suites:
  `cargo test -p iroha_data_model axt`,
  `cargo test -p ivm --test axt_host_flow`,
  `cargo test -p iroha_core --test ivm_corehost_axt`, and
  `cargo test -p integration_tests --test native_amx_routing`.
- Archive chaos-drill evidence under `ops/drill-log.md`.
- Each acceptance record includes the slot SLO report, outstanding error spikes,
  policy snapshot version, and latest AXT proof-cache snapshot.

This file is the source-adjacent reference for AMX/AXT formats, runtime signals,
and validation evidence. Public tutorials and operator walkthroughs belong in
the sibling `iroha-docs` repository.
