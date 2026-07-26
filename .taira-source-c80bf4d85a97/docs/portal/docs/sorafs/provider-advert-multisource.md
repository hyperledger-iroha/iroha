# Multi-Source Provider Adverts & Scheduling

This page distils the canonical spec in
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Use that document for verbatim Norito schemas and changelogs; the portal copy
keeps operator guidance, SDK notes, and telemetry references close to the rest
of the SoraFS runbooks.

## Norito schema additions

### Range capability (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – largest contiguous span (bytes) per request, `≥ 1`.
- `min_granularity` – seek resolution, `1 ≤ value ≤ max_chunk_span`.
- `supports_sparse_offsets` – permits non-contiguous offsets in one request.
- `requires_alignment` – when true, offsets must align with `min_granularity`.
- `supports_merkle_proof` – indicates PoR witness support.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` enforce canonical encoding
so gossip payloads remain deterministic.

### `StreamBudgetV1`
- Fields: `max_in_flight`, `max_bytes_per_sec`, optional `burst_bytes`.
- Validation rules (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, when present, must be `> 0` and `≤ max_bytes_per_sec`.

### `TransportHintV1`
- Fields: `protocol: TransportProtocol`, `priority: u8` (0–15 window enforced by
  `TransportHintV1::validate`).
- Known protocols: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Duplicate protocol entries per provider are rejected.

### `ProviderAdvertBodyV1` additions
- Optional `stream_budget: Option<StreamBudgetV1>`.
- Optional `transport_hints: Option<Vec<TransportHintV1>>`.
- Both fields now flow through `ProviderAdmissionProposalV1`, governance
  envelopes, CLI fixtures, and telemetric JSON.

## Validation & governance binding

`ProviderAdvertBodyV1::validate` and `ProviderAdmissionProposalV1::validate`
reject malformed metadata:

- Range capabilities must decode and satisfy span/granularity limits.
- Stream budgets / transport hints require a matching
  `CapabilityType::ChunkRangeFetch` TLV and non-empty hint list.
- Duplicate transport protocols and invalid priorities raise validation
  errors before adverts are gossiped.
- Admission envelopes compare proposal/adverts for range metadata via
  `compare_core_fields` so mismatched gossip payloads are rejected early.

Regression coverage lives in
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Tooling & fixtures

- Provider advert payloads must include `range_capability`, `stream_budget`, and
  `transport_hints` metadata. Validate via `/v1/sorafs/providers` responses and
  admission fixtures; JSON summaries should include the parsed capability,
  stream budget, and hint arrays for telemetry ingestion.
- `cargo xtask sorafs-admission-fixtures` surfaces stream budgets and transport
  hints inside its JSON artefacts so dashboards track feature adoption.
- Fixtures under `fixtures/sorafs_manifest/provider_admission/` now include:
  - canonical multi-source adverts,
  - `multi_fetch_plan.json` so SDK suites can replay a deterministic multi-peer
    fetch plan.

## Orchestrator & Torii integration

- Torii `/v1/sorafs/providers` returns parsed range capability metadata along
  with `stream_budget` and `transport_hints`. Downgrade warnings fire when
  providers omit the new metadata, and gateway range endpoints enforce the same
  constraints for direct clients.
- The multi-source orchestrator (`sorafs_car::multi_fetch`) now enforces range
  limits, capability alignment, and stream budgets when assigning work. Unit
  tests cover chunk-too-large, sparse‑seek, and throttling scenarios.
- `sorafs_car::multi_fetch` streams downgrade signals (alignment failures,
  throttled requests) so operators can trace why specific providers were
  skipped during planning.

## Telemetry reference

The Torii range fetch instrumentation feeds the **SoraFS Fetch Observability**
Grafana dashboard (`dashboards/grafana/sorafs_fetch_observability.json`) and the
paired alert rules (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Providers advertising range capability features. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | Throttled range fetch attempts grouped by policy. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | Active guarded streams consuming the shared concurrency budget. |

Example PromQL snippets:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Use the throttle counter to confirm quota enforcement before enabling
multi-source orchestrator defaults, and alert when concurrency approaches the
stream budget maxima for your fleet.
