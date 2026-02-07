---
lang: zh-hans
direction: ltr
source: docs/source/sorafs_orchestrator_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8584e9225df6ce386ba1ab8ac08362e22379a275fbb4c4e513ccff10d8b7d353
source_last_modified: "2026-01-22T14:35:37.572314+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Multi-Source Fetch Orchestrator Specification
summary: Canonical API, scoring model, and configuration knobs for the SF-6b multi-source fetch orchestrator.
---

# 1. Purpose and Scope

The SoraFS multi-source fetch orchestrator (SF-6b) is the deterministic client-side
scheduler that retrieves Kotodama payload chunks from multiple providers in parallel.
It consumes manifests, provider advertisements, and runtime telemetry to build a
scoreboard, assigns weights to each provider, and drives an async fetch loop that
verifies every returned chunk before exposing it to higher layers.

This specification codifies the orchestrator’s API surface, scoring model, failover
behaviour, and configuration knobs. It is source-of-truth for the Rust implementation
(`crates/sorafs_car/src/multi_fetch.rs`) and all SDK bindings generated from it.

## 1.1 Goals

- Deliver deterministic chunk retrieval that honours the manifest ordering and proof
  requirements defined by the SoraFS chunker profiles.
- Encode provider preference using a manifest-derived scoreboard so governance,
  staking, and telemetry signals influence scheduling without introducing
  non-determinism.
- Expose ergonomic async APIs for Rust and downstream language bindings while
  keeping the scheduling policy pluggable.
- Surface configuration knobs that callers can tune without recompiling the crate.
- Emit structured telemetry so observability dashboards (SF-7) can reason about retry
  loops, provider health, and SLA breaches.

## 1.2 Non-goals

- Designing the governance processes that populate provider adverts or manifest
  policies: those remain covered by the SF-2* roadmap items.
- Defining chunk attestations or PoR sampling trees: those are handled by the chunker
  (`crates/sorafs_chunker`) and pin registry specifications.
- Providing WASM bindings or browser runtimes. The orchestrator targets native
  runtimes only.

# 2. Inputs and Scoreboard

The orchestrator transforms static and dynamic signals into a `Scoreboard` that
assigns a normalised weight to each provider. The scoreboard is calculated on every
run before scheduling begins and persisted for observability under
`$STATE_DIR/sorafs_orchestrator_scoreboard.json`.

## 2.1 Data sources

| Source | Fields | Notes |
| ------ | ------ | ----- |
| Manifest (`CarBuildPlan` + `ManifestPolicyV1`) | `chunk_profile`, replication metadata, staking weight, capability requirements | Directly available from the `CarBuildPlan` returned by `sorafs_car`. |
| Provider adverts (`ProviderAdvertBodyV1`) | `range_capability`, `stream_budget`, QoS score, capability flags, governance TTLs | Must be decoded into `ProviderMetadata` before scoring. |
| Runtime telemetry (optional) | Latency EWMA, failure counters, token acceptance rate, GAR penalties | Sourced from previous orchestrator runs or observability feeds. |
| Governance overrides | Suspensions, staged rollouts, manual weight multipliers | Delivered via Norito payloads in the governance registry. |

## 2.2 Normalisation

Each provider receives a raw score `s_raw` composed of the following factors:

```
qo_component         = clamp(qo_score / 100.0, 0.1, 1.0)
latency_component    = clamp(1.0 - (latency_p95_ms / LATENCY_CAP_MS), 0.1, 1.0)
failure_component    = clamp(1.0 - failure_rate_ewma, 0.0, 1.0)
token_health_factor  = if token_health >= 0.8 { 1.0 } else { token_health }  // linear penalty
staking_multiplier   = clamp(staking_weight, 0.5, 3.0)
telemetry_penalty    = if telemetry_penalties { 0.0 } else { 1.0 }

s_raw = qo_component
      * latency_component
      * failure_component
      * token_health_factor
      * staking_multiplier
      * telemetry_penalty
```

`LATENCY_CAP_MS` defaults to 5_000 ms and may be overridden via configuration.

Scores are then normalised across all providers:

```
s_norm_i = s_raw_i / Σ s_raw_j   (with fallback to equal weights if Σ = 0)
```

The orchestrator converts `s_norm_i` into integer weights by multiplying with
`WEIGHT_SCALE` (default `10_000`) and rounding up to at least `1` so that every
eligible provider remains selectable by the scheduler.

## 2.3 Capability gating

Before a provider enters the scoreboard it must satisfy the manifest’s capability
requirements:

- Possess a `range_capability` descriptor with `max_chunk_span` ≥ the largest chunk
  in the plan.
- Honour alignment expectations if `requires_alignment` is true.
- Advertise a `stream_budget` large enough for the plan’s largest chunk.
- Remain within governance validity windows (`refresh_deadline`, `expires_at`).

Ineligible providers are reported in the scoreboard file with an explicit reason and
never scheduled until metadata is updated.

# 3. Provider Health and Failover

During execution the orchestrator tracks per-provider state (`ProviderState`):

- `capacity`: Effective concurrency derived from the provider’s configured
  `max_concurrent_chunks` and optional `stream_budget.max_in_flight`.
- `bytes_inflight`: Guardrail for burst budgets when `stream_budget.burst_bytes`
  is present.
- `consecutive_failures`: Incremented on every failed attempt; reset on success.
- `disabled`: Flag set when `consecutive_failures` reaches the configured
  `provider_failure_threshold`.

Failover policy:

- A provider marked as `disabled` is skipped for the remainder of the session to
  guarantee deterministic retry ordering. Future sessions reload the scoreboard
  afresh using updated telemetry.
- If all providers become disabled while chunks remain, the orchestrator raises
  `MultiSourceError::NoHealthyProviders` (`chunk_index`, attempt count, and last
  error are included).
- Retry attempts per chunk are bounded by `FetchOptions::per_chunk_retry_limit`.
  When the limit is exceeded the orchestrator returns
  `MultiSourceError::ExhaustedRetries`.

# 4. API Surface

## 4.1 Rust types

The Rust API lives in `crates/sorafs_car/src/multi_fetch.rs` and exposes the
following types (non-exhaustive list of significant items):

| Type / Function | Purpose |
| --------------- | ------- |
| `FetchProvider` | Static provider configuration including weight, concurrency, and optional metadata. |
| `ProviderMetadata` | Structured form of provider advert data consumed by capability checks. |
| `FetchOptions` | Runtime configuration knobs (verification flags, retry limits, global concurrency caps). |
| `FetchRequest` / `ChunkResponse` | Payload passed to caller-supplied fetchers and their responses. |
| `ChunkObserver` | Optional streaming callback invoked after each verified chunk. |
| `FetchOutcome` | Aggregate result containing chunk payloads, receipts, and per-provider reports. |
| `fetch_plan_parallel` | Primary entry point that consumes a `CarBuildPlan`, provider list, and async fetcher. |
| `fetch_plan_parallel_with_observer` | Variant that enables streamed delivery via `ChunkObserver`. |
| `MultiSourceError` | Error enum covering capability mismatches, retry exhaustion, disabled providers, and observer failures. |

Bindings for TypeScript and Go MUST wrap the same semantics without altering
ordering or verification steps. FFI surfaces shall mirror the Rust types described
above and forward `MultiSourceError` variants as structured error payloads.

The high-level facade lives in `crates/sorafs_orchestrator`, exposing an
`Orchestrator` type that bundles scoreboard construction, persistence, and fetch
helpers (with optional observers). Downstream services should depend on this crate
rather than calling `fetch_plan_parallel` directly unless they need lower-level
control. `OrchestratorConfig` includes a `telemetry_region` knob so SDKs can stamp
metrics emitted by the orchestrator with deployment-specific labels.

## 4.2 Scoreboard builder API

The `Scoreboard` builder lives in `crates/sorafs_car/src/scoreboard.rs` and
emits a `Scoreboard`. Callers obtain providers via `Scoreboard::into_providers()`.
Relevant types:

```rust
pub struct ScoreboardConfig {
    pub latency_cap_ms: u32,
    pub weight_scale: NonZeroU32,
    pub telemetry_grace_period: Duration,
    pub persist_path: Option<PathBuf>,
    pub now_unix_secs: u64,
}

pub struct ScoreboardEntry {
    pub provider: FetchProvider,
    pub raw_score: f64,
    pub normalised_weight: f64,
    pub eligibility: Eligibility,
}

pub enum Eligibility {
    Eligible,
    Ineligible(IneligibilityReason),
}

pub enum IneligibilityReason {
    MissingProviderId,
    Capability(CapabilityMismatch),
    RefreshDeadlineElapsed { refresh_deadline: u64 },
    Expired { expires_at: u64 },
    TelemetryPenalty,
    TelemetryStale { last_updated: u64 },
}

pub fn build_scoreboard(
    plan: &CarBuildPlan,
    adverts: &[ProviderMetadata],
    telemetry: &TelemetrySnapshot,
    config: &ScoreboardConfig,
) -> io::Result<Scoreboard>;
```

`TelemetrySnapshot` aggregates the runtime signals defined in §2.1. Implementations
MUST be deterministic: identical inputs yield identical weights and eligibility
decisions. `Scoreboard::persist_to_path()` writes the persisted artefact when a
`persist_path` is supplied in `ScoreboardConfig`, and `Scoreboard::into_providers()`
exposes the filtered vector of eligible providers for direct use by the
orchestrator.

# 5. Configuration Knobs

Configuration is split between the scoreboard and runtime fetch loop.

## 5.1 Scoreboard configuration

| Field | Default | Description |
| ----- | ------- | ----------- |
| `latency_cap_ms` | `5_000` | Maximum latency used when normalising the latency component. Values above the cap degrade linearly toward `0.1`. |
| `weight_scale` | `10_000` | Multiplier used when transforming normalised scores into integer weights. |
| `telemetry_grace_period` | `900` seconds | Window during which stale telemetry is tolerated before a provider is marked ineligible. |
| `persist_path` | `$STATE_DIR/sorafs_orchestrator_scoreboard.json` | Destination for persisted scoreboard artefacts when set; omitted entries skip persistence. |
| `now_unix_secs` | `SystemTime::now()` | Snapshot timestamp used when checking advert refresh/expiry deadlines. |

## 5.2 Fetch options

The defaults shipped in `FetchOptions::default()` are normative:

| Field | Default | Behaviour |
| ----- | ------- | --------- |
| `verify_lengths` | `true` | Reject any chunk whose length deviates from the plan. |
| `verify_digests` | `true` | Recompute BLAKE3 digests and refuse mismatches. |
| `per_chunk_retry_limit` | `Some(3)` | Maximum attempts per chunk before aborting. |
| `provider_failure_threshold` | `3` | Disable provider within the session after this many consecutive failures. |
| `global_parallel_limit` | `None` | Optional hard cap on concurrent requests; defaults to the sum of provider capacities. |
| `score_policy` | `None` | Deterministic hook that can adjust provider priority or veto eligibility during scheduling. |

Callers may override these fields, but doing so MUST remain deterministic across
identical inputs.

## 5.3 Orchestrator configuration

| Field | Default | Description |
| ----- | ------- | ----------- |
| `telemetry_region` | `None` | Optional label attached to orchestrator telemetry; defaults to `unspecified` when omitted. |
| `anonymity_policy` | `anon-guard-pq` (`stage-a`) | Staged SoraNet policy (`stage-a`/`anon-guard-pq`, `stage-b`/`anon-majority-pq`, `stage-c`/`anon-strict-pq`) controlling relay selection and telemetry rollouts. |

# 6. Execution Flow

1. **Scoreboard build**
   - Ingest `CarBuildPlan`, provider adverts, and telemetry.
   - Filter ineligible providers (capability mismatches, expired adverts, telemetry
     penalties) and compute weights per §2.
   - Persist the resulting scoreboard artefact for operators.
2. **Provider registry**
   - Transform eligible scoreboard entries into `FetchProvider` instances, applying
     `with_weight` and `with_metadata`.
3. **Scheduling loop**
   - Instantiate `ProviderState` for each provider and seed the weighted
     round-robin credits.
   - Iterate through `plan.chunks`, issuing fetch jobs while respecting provider
     capacity and global limits.
   - On response, run verification checks (length + digest) before yielding to the
     observer.
4. **Observer delivery**
   - If a `ChunkObserver` is registered, call `on_chunk` strictly in chunk-index order.
     Observer failures abort the session with `MultiSourceError::ObserverFailed`.
5. **Outcome aggregation**
   - Collect verified chunk buffers, provider receipts, and per-provider reports.
   - Return `FetchOutcome` to the caller.

All concurrency runs on Tokio 1.x (the canonical runtime for the workspace). Tasks
inherit the caller’s tracing span so downstream instrumentation can correlate metrics.

# 7. Observability

The orchestrator emits the following metrics via `iroha_telemetry` exporters:

- `sorafs_orchestrator_active_fetches{manifest_id,region}`
- `sorafs_orchestrator_fetch_duration_ms{manifest_id,region}`
- `sorafs_orchestrator_fetch_failures_total{manifest_id,region,reason}`
- `sorafs_orchestrator_retries_total{manifest_id,provider_id,reason}`
- `sorafs_orchestrator_provider_failures_total{manifest_id,provider_id,reason}`
- `sorafs_orchestrator_chunk_latency_ms{manifest_id,provider_id}`
- `sorafs_orchestrator_bytes_total{manifest_id,provider_id}`
- `sorafs_orchestrator_stalls_total{manifest_id,provider_id}`
- `sorafs_orchestrator_policy_events_total{region,stage,outcome,reason}`
- `sorafs_orchestrator_pq_ratio{region,stage}`
- `sorafs_orchestrator_pq_candidate_ratio{region,stage}`
- `sorafs_orchestrator_pq_deficit_ratio{region,stage}`
- `sorafs_orchestrator_brownouts_total{region,stage,reason}`

Structured telemetry events are emitted on the `telemetry::sorafs.fetch.*` targets:

- `telemetry::sorafs.fetch.lifecycle` — lifecycle updates with `event=start|complete`, `status`,
  `manifest`, `job_id`, `region`, `chunk_count`, retry totals, elapsed `duration_ms`, `total_bytes`,
  average throughput (MiB/s), observed stall count, PQ supply/deficit ratios, and a boolean
  indicating whether the staged policy should be treated as an effective brownout.
- `telemetry::sorafs.fetch.retry` — per-provider retry observations (`provider`, `reason`,
  `attempts`).
- `telemetry::sorafs.fetch.provider_failure` — provider session failure counters surfaced with the
  failure `reason` and count.
- `telemetry::sorafs.fetch.error` — terminal failures including the orchestrator `reason` and last
  provider failure (if any).
- `telemetry::sorafs.fetch.stall` — raised whenever a chunk exceeds `ScoreboardConfig::latency_cap_ms`,
  capturing the offending provider, latency, and bytes transferred.

Lifecycle, success, and error events now also embed the staged anonymity policy (`stage`),
outcome (`met|brownout|not_applicable`), fallback reason, PQ supply ratio, deficit ratio, and the
effective brownout flag so downstream collectors can gate rollouts during the SNNet-5b ratchet.

Events reuse the same redaction rules as other telemetry targets and attach the randomly generated
`job_id` so downstream collectors can stitch lifecycle, retry, and error records together. The
`SorafsFetchOtel` exporter mirrors the metrics above onto the OpenTelemetry pipeline, ensuring both
Prometheus scrapes and OTLP traces share the same manifest/job identifiers. PQ adoption is tracked
via the candidate ratio histogram; deficit ratios and brownout counters expose the path build delta
requested for SNNet-5b (including `no_soranet` inventory gaps).

Structured logs (JSON) still include provider ID, chunk index, attempt number, and error category.
Telemetry MUST distinguish between capability refusals (static) and runtime failures (dynamic) to aid
governance triage.

# 8. SDK and CLI Integration

- **Rust** exposes `Orchestrator::fetch` helpers atop `fetch_plan_parallel` for
  idiomatic use in the CLI and service daemons.
- **TypeScript** bindings use `napi-rs`, returning an async iterator whose items are
  `{ chunkIndex, bytes, providerId, attempts }`.
- **Go** bindings wrap the FFI surface and surface results on a channel alongside a
  context-aware error channel.
- The CLI command `iroha app sorafs fetch` accepts a manifest (`--manifest`), chunk plan (`--plan`),
  gateway descriptors (`--gateway-provider=*`), and surfaces `--max-peers` / `--retry-budget`
  knobs while emitting human-readable receipts and JSON summaries.
- The Rust SDK (`crates/iroha`) exposes `Client::sorafs_fetch_via_gateway` with
  configurable retry budgets and provider limits, wiring gateway manifests and stream tokens into
  the orchestrator while tagging metrics with the client's chain identifier.

All bindings MUST call into the Rust implementation without diverging from the
ordering, verification, or telemetry semantics described above.

# 10. Open Items

- Integrate telemetry snapshots from SF-7 once the observability pipeline settles.
- Add governance hooks that allow emergency suspensions to short-circuit the
  scoreboard builder.
- Extend the scoreboard artefact schema with historical trend data once retention
  requirements are finalised.

# 11. Binding polish checklist (GA readiness)

| Item | Owner(s) | Status | Notes |
|------|----------|--------|-------|
| CLI UX sweep (`sorafs_fetch`, `sorafs_cli orchestrator …`) | Tooling WG / Docs | 🈴 Completed | Flag descriptions now live in `docs/source/sorafs/snippets/multi_source_flag_notes.txt` and are embedded via `include_str!` in `sorafs_fetch` plus `{{#include}}` in the docs, keeping `--max-peers`, `--retry-budget`, observer hooks, and telemetry note text in lock-step. |
| TypeScript binding refresh | JS SDK team | 🈴 Completed | `sorafsGatewayFetch` now raises `SorafsGatewayFetchError` with the structured `MultiSourceError` payload (`javascript/iroha_js/src/sorafs.js`) and the bindings/types (`javascript/iroha_js/index.d.ts`) expose the same shape. `ToriiClient` gained a `retryTelemetryHook` (`javascript/iroha_js/src/toriiClient.js`, `src/config.js`) documented under `docs/source/sdk/js/index.md`, so JS callers share the same retry telemetry surface as the CLI. |
| Swift async wrapper polish | Swift SDK team | 🈴 Completed | `SorafsOrchestratorClient` now lives in `IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift`, exposing async `fetch`/`fetchRaw` helpers plus typed `SorafsGatewayFetchReport`. The parity test rewrites (`IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`) consume the new client and keep the normalized report comparison used by Rust/JS. |
| GA parity ledger | Program Management | ⏳ In Progress | Extend `docs/source/sorafs/reports/orchestrator_ga.md` with a binding matrix (Rust/JS/Swift) that records commit hashes for observer support, telemetry relays, and retry knobs. |

This checklist feeds the SF‑6b row in `roadmap.md` so CLI/SDK binding polish
stays visible until every box is checked.
