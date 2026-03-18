---
lang: ar
direction: rtl
source: docs/source/sorafs/provider_advert_multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ac5179e81ce3ac75da53d9ed8164496cca86cf005b362dc8261025c4a8f69ab8
source_last_modified: "2026-01-04T10:50:53.659900+00:00"
translation_last_reviewed: 2026-01-30
---

# Provider Advert Extensions for Multi-Source Scheduling (SF-2d)

This note records the schema work, validations, and tooling changes needed for
multi-source chunk scheduling in SoraFS. The Norito payloads now surface range
fetch metadata, throughput budgets, and transport preferences so clients can

## Norito Schema

### `CapabilityType::ChunkRangeFetch` (`ProviderCapabilityRangeV1`)
- `max_chunk_span`: largest contiguous block (bytes) a provider will ship in a
  single ranged request. Must be ≥ 1.
- `min_granularity`: smallest seek resolution within a chunk. Must be ≥ 1 and
  ≤ `max_chunk_span`.
- `supports_sparse_offsets`: indicates non-contiguous offsets are legal within
  one request.
- `requires_alignment`: when `true`, offsets must align to
  `min_granularity`.
- `supports_merkle_proof`: provider can attach proof material alongside ranged
  responses.
- Encoding helpers: `ProviderCapabilityRangeV1::to_bytes` /
  `from_bytes` ensure canonical Norito payloads.

### `StreamBudgetV1`
- Fields: `max_in_flight`, `max_bytes_per_sec`, optional `burst_bytes`.
- Validation rules (enforced by `StreamBudgetV1::validate` and exercised in
  `crates/sorafs_manifest/src/provider_advert.rs` tests):
  - `max_in_flight` ≥ 1, `max_bytes_per_sec` > 0.
  - `burst_bytes`, when present, must be > 0 and ≤ `max_bytes_per_sec`.

### `TransportHintV1`
- Fields: `protocol: TransportProtocol`, `priority: u8`.
- Priority window is 0–15 (enforced via `TransportHintV1::validate` and CLI
  parsing). Priority collisions are rejected per provider.
- Supported protocols: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.

### `ProviderAdvertBodyV1` additions
- Optional `stream_budget: Option<StreamBudgetV1>`.
- Optional `transport_hints: Option<Vec<TransportHintV1>>`.
- Both fields propagate through `ProviderAdmissionProposalV1`, the governance
  envelope, CLI fixtures, and telemetry JSON outputs.

## Validation & Governance Binding

`ProviderAdvertBodyV1::validate` and `ProviderAdmissionProposalV1::validate`
now enforce:
- Range capability payloads must decode and satisfy span/granularity limits.
- Stream budgets and transport hints must pass their respective validators.
- Stream budgets / transport hints require an accompanying `CapabilityType::ChunkRangeFetch`
  TLV, and transport hint lists must be non-empty.
- Duplicate `TransportProtocol` entries are rejected.
- Admission envelopes compare proposal/adverts for stream budgets and transport
  hints (`compare_core_fields`), preventing mismatched gossip payloads.

Test coverage highlights:
- `crates/sorafs_manifest/src/provider_advert.rs` validates edge cases for
  stream budgets, transport hint priorities, duplicate protocols, and malformed
  range payloads.
- `crates/sorafs_manifest/src/provider_admission.rs` ensures governance fails
  when stream budgets/hints diverge and rejects invalid hint priorities.

## Tooling & Fixtures

- Provider advert payloads must include `range_capability`, `stream_budget`, and
  `transport_hints` metadata. Validate via `/v1/sorafs/providers` responses and
  admission fixtures; JSON reports should surface the structured `range`,
  `stream_budget`, and `transport_hints` sections for telemetry.
- `cargo xtask sorafs-admission-fixtures` emits JSON summaries that now expose
  stream budgets and transport hints for telemetry dashboards.
- `crates/sorafs_car/src/bin/provider_admission_fixtures.rs` and
  `crates/sorafs_car/src/bin/sorafs_fetch.rs` fixtures/tests were updated to
  use canonical range capability encodings and to exercise success/downgrade
  paths with the new metadata.

## Remaining Integration Work

- Torii `/v1/sorafs/providers` responses now include parsed range capability
  metadata plus `stream_budget` and `transport_hints`, and emit downgrade
  warnings when providers omit the new fields. Gateway range endpoints enforce
  the same constraints, with remaining work focused on scheduler telemetry and
  token integration.
- The multi-source orchestrator (`sorafs_car::multi_fetch`) now enforces the new
  metadata when assigning jobs: range-capability limits gate provider
  eligibility, alignment violations bubble up as explicit warnings, and stream
  budgets clamp concurrency, with unit tests covering chunk-too-large,
  length/offset mismatches, capability absence, and single-stream throttling.【crates/sorafs_car/src/multi_fetch.rs:190-262】【crates/sorafs_car/src/multi_fetch.rs:456-520】【crates/sorafs_car/src/multi_fetch.rs:1341-1501】
- Fixtures in `fixtures/sorafs_manifest/provider_admission/` include both modern
  multi-fetch plan for SDK integration tests (`multi_fetch_plan.json`).【fixtures/sorafs_manifest/provider_admission/README.md:1】
- Torii now exports range fetch observability metrics —
  `torii_sorafs_provider_range_capability_total` (feature-labelled gauge),
  `torii_sorafs_range_fetch_throttle_events_total` (throttle reasons), and
  `torii_sorafs_range_fetch_concurrency_current` (active guarded streams) — and
  the **SoraFS Fetch Observability** Grafana dashboard surfaces capability mix,
  throttle rates, and live concurrency for on-call use.

## Telemetry Reference

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Counts provider adverts exposing the range capability and associated features. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | Number of throttled range fetch attempts grouped by the policy that rejected the request. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | Active range fetch streams guarded by token concurrency limits. |

Grafana panels (dashboard UID `sorafs-fetch`) chart throttle rates over 5-minute
windows and expose concurrency gauges with alert thresholds. Example queries:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```
