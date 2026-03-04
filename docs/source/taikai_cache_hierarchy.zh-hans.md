---
lang: zh-hans
direction: ltr
source: docs/source/taikai_cache_hierarchy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5de9b350eb15f31f54ec4f4eca4fb89ac89138b32deea1f60d90510dbf65974
source_last_modified: "2026-01-05T09:28:12.100993+00:00"
translation_last_reviewed: 2026-02-07
---

# Taikai Cache Hierarchy

_Status: Completed (reliability gates + exit hedging live)_ — Owners: SRE / Networking TL / DA Program  
Roadmap item: **SNNet-14 — Taikai cache hierarchy & QoS enforcement**

The Taikai cache hierarchy provides the deterministic, multi-tier storage layer
for SoraNet distribution pilots. It underpins the “SNNet-14” backlog by wiring a
production-grade cache into the `sorafs_orchestrator` crate with configurable
capacity, retention, and QoS guarantees.

## Highlights

- **Three-tier cache (hot/warm/cold).** Each tier exposes independent capacity
  and retention settings so live segments remain in hot storage, recently
  accessed material spills into warm storage, and archival content persists in
  the cold tier. A lightweight LRU implementation handles promotion/demotion
  and eviction tracking (`CacheEviction`, `CachePromotion`).
- **QoS token buckets.** The cache enforces per-class
  (priority/standard/bulk) budgets using deterministic token buckets
  (`QosEnforcer`) so hot/warm/cold tiers admit/promote traffic deterministically.
  Queue dispatch no longer consumes shaper budgets; shard reliability gates
  handle routing while the cache-side QoS keeps insert and promotion pressure
  predictable.
- **Reliability gates.** Shard-level circuit breakers trip after consecutive
  failures (2 s open window by default), detour batches to healthy shards in the
  consistent hash ring, and surface their state via gauges + failover counters
  so brownouts are visible in both JSON summaries and Grafana.
- **Exit hedging.** The Taikai pull queue now drives hedged fetches directly
  from the orchestrator wrapper: batches that sit in-flight longer than
  `hedge_after` (125 ms by default, tunable via `TaikaiPullQueueConfig`) trigger
  a second fetch attempt while the original remains live, with hedged counters
  reflected in queue stats and `taikai_cache_queue` summaries. The hedging path
  is covered by `taikai_fetch_wrapper_hedges_overdue_batches` in
  `crates/sorafs_orchestrator/src/lib.rs`.
- **Instrumentation-first.** Insert, fetch, and QoS operations return structured
  outcomes (`TaikaiCacheInsertOutcome`, `TaikaiCacheQueryOutcome`, `QosError`)
  so the orchestrator can surface eviction causes, promotion trails, and rate
  limit events in telemetry.
- **Consistent hashing.** `TaikaiShardRing` exposes a virtual-node consistent
  hash for shard placement. As SNNet-14’s CAA gossip network comes online the
  same ring will drive shard assignment and rebalancing.
- **CAA gossip records.** `CacheAdmissionRecord` + `CacheAdmissionEnvelope`
  capture hot/warm/cold admissions and evictions, include deterministic payload
  digests, TTL metadata, and are signed with guard-directory keys. Gossip
  announcements wrap the envelope in `CacheAdmissionGossipBody`/`CacheAdmissionGossip`
  with a nonce + independent expiry, and `CacheAdmissionReplayFilter` enforces a
  deterministic replay window before caches act on the message (see
  `crates/sorafs_orchestrator/src/taikai_cache.rs`).
- **Chunk-level Taikai hints.** `ChunkFetchSpec` now exposes an optional
  `taikai_segment_hint` block (JSON + binary) so Taikai manifests can tag the
  CMAF segments that should flow through the queue. The orchestrator only
  enables the queue bridge when at least one hint is present, avoiding any
  impact on generic SoraFS fetches; DA manifests now propagate the Taikai
  metadata directly into the fetch plan so hints arrive automatically when
  Torii returns Taikai segments.
- **CAA-driven shard ring.** `CacheAdmissionTracker` ingests
  `CacheAdmissionGossip`, maintains a replay window, expires stale entries, and
  rewrites the pull-queue shard ring so hedging and failover follow the same
  cache admission topology advertised on the gossip plane.

## Configuration

The orchestrator now accepts `taikai_cache` inside its JSON bindings. The
configuration mirrors `TaikaiCacheConfig`:

```json
{
  "taikai_cache": {
    "hot_capacity_bytes": 8388608,
    "hot_retention_secs": 45,
    "warm_capacity_bytes": 33554432,
    "warm_retention_secs": 180,
    "cold_capacity_bytes": 268435456,
    "cold_retention_secs": 3600,
    "qos": {
      "priority_rate_bps": 83886080,
      "standard_rate_bps": 41943040,
      "bulk_rate_bps": 12582912,
      "burst_multiplier": 4
    }
  }
}
```

All fields are optional in the orchestrator payload: omitting the section
disables the cache, while individual keys default to the tuned values shipped
in `TaikaiCacheConfig::default`. The orchestrator exposes the live handle via
`Orchestrator::taikai_cache()` so upcoming SNNet-14 workstreams can integrate
CAA gossip, exit hedging, and observability sampling.

## CAA Gossip Plane (SNNet-14A)

The cache now emits deterministic announcement records whenever a segment is
admitted or evicted from a tier. Each announcement is represented by a
`CacheAdmissionRecord` (segment key, tier, QoS class, payload digest/size,
issuer shard, issued/expiry timestamps) and wrapped in a
`CacheAdmissionEnvelope` that carries the guard directory identifier, signer,
and detached signature:

```rust
let record = CacheAdmissionRecord::from_segment(
    TaikaiShardId(3),
    GuardDirectoryId::new("soranet/canary"),
    &cached_segment,
    CacheTierKind::Hot,
    CacheAdmissionAction::Admit,
    issued_unix_ms,
    Duration::from_secs(30),
)?;
let envelope = CacheAdmissionEnvelope::sign(record, guard_key_pair)?;
envelope.verify(now_unix_ms)?;
```

- Records enforce bounded TTLs (`issued_unix_ms + ttl`) so stale entries are
  rejected automatically.
- Envelopes must pass signature verification before caches or orchestrators act
  on them; failed verifications surface `CacheAdmissionError::InvalidSignature`.
- Norito serialization/deserialization keeps the format deterministic for both
  gossip payloads and storage.
- Gossip announcements wrap envelopes with a nonce + independent TTL (`CacheAdmissionGossipBody`
  → `CacheAdmissionGossip::sign/verify`), and the replay filter
  (`CacheAdmissionReplayFilter`) rejects duplicates during a configurable
  window so peers cannot flood the plane with replays:

```rust
let gossip_body =
    CacheAdmissionGossipBody::new(envelope.clone(), issued_unix_ms, Duration::from_secs(15))?;
let gossip = CacheAdmissionGossip::sign(gossip_body, guard_key_pair)?;
gossip.verify(now_unix_ms)?;
let mut replay = CacheAdmissionReplayFilter::new(Duration::from_millis(500), 128)?;
if replay.observe(&gossip, now_unix_ms)? {
    // safe to process admission/eviction event
}
```

Integration tests covering round-trips, expiry handling, and tampering live in
`crates/sorafs_orchestrator/tests/taikai_cache.rs`, ensuring future changes to

## Telemetry

Instrumentation now feeds the shared `iroha_telemetry` registry so dashboards
can track cache efficiency alongside the existing `sorafs.fetch.*` panels. The
following Prometheus series (with OTLP mirrors) are emitted:

- `sorafs_taikai_cache_query_total{result="hit|miss",tier}` — hit/miss counts
- `sorafs_taikai_cache_insert_total{tier}` — insert events per tier, paired
  with `sorafs_taikai_cache_bytes_total{event="insert|hit",tier}` to expose byte
  throughput
- `sorafs_taikai_cache_evictions_total{tier,reason}` — capacity/expiry evictions
- `sorafs_taikai_cache_promotions_total{from_tier,to_tier}` — warm/cold promotions
- `sorafs_taikai_qos_denied_total{class}` — QoS bucket denials per class
- `sorafs_taikai_queue_depth{state}` — queue depth gauges (pending segments/bytes/batches,
  in-flight batches, open circuits)
- `sorafs_taikai_queue_events_total{event,class}` — queue events (issued, hedged, rate-limited,
  backpressure) grouped by QoS class
- `sorafs_taikai_shard_circuits_open{shard}` and
  `sorafs_taikai_shard_failovers_total{preferred_shard,selected_shard}` —
  reliability gauges/counters for the pull queue circuit breakers.

These metrics show up in the SoraFS fetch observability pack
(`dashboards/grafana/sorafs_fetch_observability.json`) so operators can alarm on
miss/brownout rates during the SNNet-14 rollout. The Taikai cache dashboard
(`dashboards/grafana/taikai_cache.json`) now includes dedicated panels for open
circuits, queue depth, hedging/rate-limit events, and failover rates so SREs
can correlate brownouts with shard-level instability.

## Operator Controls & Rollout Overrides

### CLI (`sorafs_cli fetch`)

`sorafs_cli fetch` now accepts a dedicated `--taikai-cache-config=PATH` flag so
pilots can enable or tune the cache without editing the base orchestrator
configuration. The flag expects a JSON payload that mirrors
`TaikaiCacheConfig`—either a standalone object or the full orchestrator JSON
with a `taikai_cache` section. Example:

```json
{
  "hot_capacity_bytes": 8388608,
  "hot_retention_secs": 45,
  "warm_capacity_bytes": 33554432,
  "warm_retention_secs": 180,
  "cold_capacity_bytes": 268435456,
  "cold_retention_secs": 3600,
  "qos": {
    "priority_rate_bps": 83886080,
    "standard_rate_bps": 41943040,
    "bulk_rate_bps": 12582912,
    "burst_multiplier": 4
  },
  "reliability": {
    "failures_to_trip": 3,
    "open_secs": 2
  }
}
```

Invoke the fetcher with:

```bash
sorafs_cli fetch \
  --plan=fixtures/taikai_plan.json \
  --manifest-id=deadbeef... \
  --provider name=edge-a,provider-id=...,base-url=...,stream-token=... \
  --taikai-cache-config=configs/taikai-cache/hot-warm-warmup.json \
  --output=/tmp/payload.car
```

Distributing multiple JSON snippets lets SREs pin different tier sizes per
rollout phase (canary vs. ramp). The option is plumbed through
`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`.

Reliability fields are optional; omit them to use the defaults (3 failures to
trip, 2 s open interval).

The fetch summary now includes a `taikai_cache_summary` block so operators can
inspect cache health without scraping Prometheus. Example:

```json
"taikai_cache_summary": {
  "hits": {"hot": 12, "warm": 4, "cold": 0},
  "misses": 3,
  "inserts": {"hot": 16, "warm": 2, "cold": 1},
  "evictions": {
    "hot": {"expired": 0, "capacity": 5},
    "warm": {"expired": 0, "capacity": 1},
    "cold": {"expired": 0, "capacity": 0}
  },
  "promotions": {"warm_to_hot": 4, "cold_to_warm": 0, "cold_to_hot": 0},
  "qos_denials": {"priority": 1, "standard": 0, "bulk": 0}
}
```

The queue snapshot carries the complementary reliability telemetry:

```json
"taikai_cache_queue": {
  "pending_segments": 18,
  "pending_bytes": 9437184,
  "pending_batches": 4,
  "in_flight_batches": 2,
  "hedged_batches": 1,
  "dropped_segments": 0,
  "failovers": 1,
  "open_circuits": 1
}
```

Use this snapshot together with the dashboards to correlate miss bursts, QoS
denials, and eviction churn before toggling transport policies.

### Coalesced pull/push queue

SNNet-14 now ships the first piece of the coalesced pull/push orchestration
via the `TaikaiPullQueue`. Every orchestrator instance that enables
`taikai_cache` automatically provisions:

- **Deterministic batching.** Incoming `SegmentKey`s (plus QoS/size hints) are
  coalesced into shard-aware batches (`TaikaiPullBatch`) that honour the cache
  tier limits. The queue enforces configurable `max_batch_segments`,
  `max_batch_bytes`, and `max_in_flight_batches` thresholds so upstream exits
  never see unbounded fan-out.
- **Back-pressure feedback.** When the backlog exceeds the configured ceiling
  (default: 256 pending segments) the queue fails fast with a
  `TaikaiQueueError::Backpressure` error, allowing callers to shed load or
  temporarily bypass the cache rather than overwhelming exits.
- **Exit hedging.** Batches that remain in flight longer than
  `hedge_after` (125 ms by default) are reissued with the `hedged=true`
  flag; the orchestrator fetch wrapper now respects that signal and races a
  second fetch attempt while leaving the original request active. Hedged
  counters feed directly into the CLI/Jenkins JSON summary.
- **Circuit breakers + failover.** Three consecutive failures against a shard
  trip a 2 s circuit breaker. While open, the queue reassigns batches to the
  next healthy shard in the consistent hash ring and records both the open
  state (`sorafs_taikai_shard_circuits_open`) and the failover path
  (`sorafs_taikai_shard_failovers_total`) so brownouts are visible to SREs.
- **Shard ring integration.** `TaikaiCacheHandle::configure_shards` swaps the
  consistent hash ring in place, so upcoming CAA gossip work can rebalance
  cache tiers without rewriting the queue plumbing.
- **Observability hooks.** `FetchSession` now captures both
  `taikai_cache_summary` (per-tier hits/misses) and `taikai_cache_queue`
  (pending segments/bytes/batches, hedged batches, drops, failovers, open
  circuits). `sorafs_cli --json-out` emits the updated `taikai_cache_queue`
  block and the Grafana pack mirrors the same fields, giving SREs real-time
  visibility into queue depth, hedging behaviour, and shard health without the
  now-removed shaping counters.

#### Orchestrator integration hooks

`Orchestrator::taikai_cache_handle()` now returns a clone of the internal
`TaikaiCacheHandle`, giving fetchers a safe facade over both the cache and the
pull queue. The handle exposes helpers for the full SN14-B workflow and the
fetch path now wires the queue directly whenever a chunk advertises a
`taikai_segment_hint` (the `ChunkFetchSpec` field emitted by Taikai manifests).
Until the ingest/manifest tooling sets those hints the queue stays idle, but as
soon as they show up the orchestrator begins issuing/cancelling queue entries
alongside the normal HTTP fetches. The new helpers include:

- `enqueue_pull(..)` adds a `TaikaiPullRequest` and surfaces
  `TaikaiQueueError::Backpressure` when the backlog exceeds the configured
  limit. Poisoned locks raise `TaikaiQueueError::Unavailable`, allowing callers
  to fall back to direct fetches if the handle becomes unhealthy.
- `issue_ready_batch[_at](..)` emits the next shard-aware `TaikaiPullBatch`,
  while `hedge_overdue_batches[_at](..)` duplicates in-flight batches with the
  `hedged=true` flag once they remain unacknowledged past `hedge_after`.
- `complete_batch(..)` frees the in-flight slot once an exit confirms delivery,
  while `fail_batch(..)` marks the shard as unhealthy and opens the circuit
  breaker/metrics when fetches error out. The fetch loop calls these
  automatically via the bridge, so operators only need to wire
  `taikai_segment_hint` into the manifests to activate the queue. The
  `wrap_fetcher_with_taikai_queue` helper now handles ticket lifecycle and
  hedging for every Taikai-tagged fetch, racing a second request when the queue
  reports an overdue batch while keeping the original request active.

Example orchestration snippet:

```rust
if let Some(handle) = orchestrator.taikai_cache_handle() {
    let key = TaikaiSegmentKey::from_envelope(&segment_envelope);
    let request = TaikaiPullRequest::new(key, TaikaiQosClass::Priority, segment_bytes);
    if let Err(TaikaiQueueError::Backpressure { .. }) = handle.enqueue_pull(request) {
        downgrade_to_single_source();
    }

    if let Ok(Some(batch)) = handle.issue_ready_batch() {
        dispatch_to_primary_exit(batch.clone());
        for hedged in handle.hedge_overdue_batches().unwrap_or_default() {
            dispatch_to_secondary_exit(hedged);
        }
        let _ = handle.complete_batch(batch.id.into());
    }
}
```

Cache admission gossip now steers the shard ring directly: the
`CacheAdmissionTracker` (available via
`Orchestrator::taikai_cache_tracker()` or
`Orchestrator::apply_cache_admission_gossip(...)`) verifies
`CacheAdmissionGossip` entries, honors replay/expiry windows, and rewrites the
consistent hash ring so hedging and failover routes follow the cache admission
topology advertised on the CAA plane.

### SDK overrides

JavaScript clients can now pass a `taikaiCache` object to
`sorafsGatewayFetch(...)`. The TypeScript surface is exported via
`SorafsTaikaiCacheOptions`/`SorafsTaikaiCacheQosOptions` (`javascript/iroha_js/index.d.ts`)
and mirrors the Rust struct:

```ts
await sorafsGatewayFetch(manifestId, chunker, planJson, providers, {
  taikaiCache: {
    hotCapacityBytes: 8_388_608,
    hotRetentionSecs: 45,
    warmCapacityBytes: 33_554_432,
    warmRetentionSecs: 180,
    coldCapacityBytes: 268_435_456,
    coldRetentionSecs: 3_600,
    qos: {
      priorityRateBps: 83_886_080,
      standardRateBps: 41_943_040,
      bulkRateBps: 12_582_912,
      burstMultiplier: 4,
    },
  },
});
```

Bindings reject invalid (zero/negative) capacities and burst multipliers, making
SDK enforcement symmetric with the CLI (`javascript/iroha_js/src/sorafs.js`,
`crates/iroha_js_host/src/lib.rs`).

Swift apps can feed the same payload through `SorafsGatewayFetchOptions` by
supplying the new `taikaiCache` property:

```swift
let cache = SorafsTaikaiCacheOptions(
    hotCapacityBytes: 8_388_608,
    hotRetentionSecs: 45,
    warmCapacityBytes: 33_554_432,
    warmRetentionSecs: 180,
    coldCapacityBytes: 268_435_456,
    coldRetentionSecs: 3_600,
    qos: SorafsTaikaiCacheQosOptions(
        priorityRateBps: 83_886_080,
        standardRateBps: 41_943_040,
        bulkRateBps: 12_582_912,
        burstMultiplier: 4
    )
)
let options = SorafsGatewayFetchOptions(taikaiCache: cache)
```

Python bindings (`iroha_python.sorafs_gateway_fetch`) accept the same structure
using a nested mapping:

```python
options = {
    "taikai_cache": {
        "hot_capacity_bytes": 8_388_608,
        "hot_retention_secs": 45,
        "warm_capacity_bytes": 33_554_432,
        "warm_retention_secs": 180,
        "cold_capacity_bytes": 268_435_456,
        "cold_retention_secs": 3_600,
        "qos": {
            "priority_rate_bps": 83_886_080,
            "standard_rate_bps": 41_943_040,
            "bulk_rate_bps": 12_582_912,
            "burst_multiplier": 4,
        },
    },
}
```

These overrides ride through the Norito bridge (`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift:4`)
and the Python gateway helper (`python/iroha_python/iroha_python_rs/src/lib.rs:2133`), ensuring
all SDKs participate in the SNNet-14 rollout without manual JSON plumbing.

### Governance DAG & staged rollout checklist

- Store approved cache profiles under the governance DAG (`configs/taikai_cache/…`)
  and reference them from the rollout manifest (`docs/source/sorafs_governance_dag_plan.md`).
- Each entry must include the JSON blob above plus the signed Norito payload so
  SoraNS relays can verify provenance before accepting the new parameters.
- When promoting a new profile, publish the DAG CID and attach the same JSON to
  the incident/runbook log so brownout drills have deterministic inputs.

The `configs/taikai_cache/profiles/` directory seeds the canonical JSON inputs
(`balanced-canary.json`, `ramp-2026q2.json`). Run
`cargo xtask sorafs-taikai-cache-bundle` to convert the profiles into
governance bundles under `artifacts/taikai_cache/<profile>/`, which contain the
canonical `profile.json`, the extracted `cache_config.json`, the Norito payload
(`profile.taikai_cache_profile.to`), and a signed-manifest-ready metadata file
(`profile.manifest.json`). The command also refreshes
`artifacts/taikai_cache/index.json`, making it easy to attach the latest hashes
to change records.

### Troubleshooting playbook

1. Use `sorafs_cli fetch --json-out …` to inspect `taikai_cache_summary` for miss
   bursts before falling back to single-source transport.
2. Watch `sorafs_taikai_cache_query_total{result="miss"}` and
   `sorafs_taikai_cache_evictions_total{reason="capacity"}`—if either spikes,
   re-check the tier sizes captured in the DAG and re-run `--taikai-cache-config`
   with the corrective JSON.
3. Verify the SDK override by correlating the fetched payload’s Norito summary
   with the governance entry (the CLI writes both artifacts into
   `artifacts/sorafs_orchestrator/<stamp>/`).
4. Record any manual overrides (CLI flag or SDK option) in the SNNet-14
   operations log within 24 h and reset to the published DAG profile once the
   incident is closed.

## Next Steps

1. *(Done)* Wire the cache outcomes into orchestrator telemetry
   (`sorafs.fetch.*`) and dashboards.
2. *(Done)* Implement the coalesced pull/push queue, expose queue stats in the
   orchestrator/CLI, keep hedging/back-pressure metrics under telemetry, and
   bridge the queue to the CAA gossip plane via
   `CacheAdmissionTracker`/`Orchestrator::apply_cache_admission_gossip(...)`.
   Taikai manifests now propagate `taikai_segment_hint` directly into fetch
   plans so the queue activates automatically when Torii returns Taikai
   segments; circuit breakers and failover telemetry (SNNet-14C) are live.
3. Add per-event slice enforcement (R_hot / R_cold) and registry integration.
4. *(Done)* Expose cache health and QoS counters via additional SDK surfaces
   beyond the CLI (JS/Python/Swift convenience wrappers now emit
   `taikai_cache_summary` + `taikai_cache_queue` snapshots alongside the
   gateway fetch reports).

These follow-on tasks remain tracked under SNNet-14 in `roadmap.md`.
