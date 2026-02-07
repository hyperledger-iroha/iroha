---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e0cdd8242b45628e688d94ebec08e2d9900787ec93a81417e6683d399d43be2d
source_last_modified: "2026-01-22T14:35:36.781385+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Node ↔ Client Protocol

This guide summarises the canonical protocol definition in
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Use the upstream spec for byte-level Norito layouts and changelogs; the portal
copy keeps the operational highlights close to the rest of the SoraFS runbooks.

## Provider Adverts & Validation

SoraFS providers gossip `ProviderAdvertV1` payloads (see
`crates/sorafs_manifest::provider_advert`) signed by the governed operator.
The adverts pin discovery metadata and the guardrails the multi-source
orchestrator enforces at runtime.

- **Lifetime** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Providers
  should refresh every 12 hours.
- **Capability TLVs** — the TLV list advertises transport features (Torii,
  QUIC+Noise, SoraNet relays, vendor extensions). Unknown codes may be skipped
  when `allow_unknown_capabilities = true`, following GREASE guidance.
- **QoS hints** — `availability` tier (Hot/Warm/Cold), maximum retrieval
  latency, concurrency limit, and optional stream budget. QoS must align with
  observed telemetry and is audited by admission.
- **Endpoints & rendezvous topics** — concrete service URLs with TLS/ALPN
  metadata plus the discovery topics clients should subscribe to when building
  guard sets.
- **Path diversity policy** — `min_guard_weight`, AS/pool fan-out caps, and
  `provider_failure_threshold` make deterministic multi-peer fetches possible.
- **Profile identifiers** — providers must expose the canonical handle (e.g.
  `sorafs.sf1@1.0.0`); optional `profile_aliases` help older clients migrate.

Validation rules reject zero stake, empty capability/endpoints/topic lists,
misordered lifetimes, or missing QoS targets. Admission envelopes compare the
advert and proposal bodies (`compare_core_fields`) before gossiping updates.

### Range Fetch Extensions

Range-capable providers include the following metadata:

| Field | Purpose |
|-------|---------|
| `CapabilityType::ChunkRangeFetch` | Declares `max_chunk_span`, `min_granularity`, and alignment/proof flags. |
| `StreamBudgetV1` | Optional concurrency/throughput envelope (`max_in_flight`, `max_bytes_per_sec`, optional `burst`). Requires a range capability. |
| `TransportHintV1` | Ordered transport preferences (e.g., `torii_http_range`, `quic_stream`, `soranet_relay`). Priorities are `0–15` and duplicates are rejected. |

Tooling support:

- Provider advert pipelines must validate range capability, stream budget, and
  transport hints before emitting deterministic payloads for audits.
- `cargo xtask sorafs-admission-fixtures` bundles canonical multi-source
  adverts alongside downgrade fixtures under
  `fixtures/sorafs_manifest/provider_admission/`.
- Range-capable adverts that omit `stream_budget` or `transport_hints` are
  rejected by the CLI/SDK loaders before scheduling, keeping the multi-source
  harness aligned with Torii admission expectations.

## Gateway Range Endpoints

Gateways accept deterministic HTTP requests that mirror the advert metadata.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Requirement | Details |
|-------------|---------|
| **Headers** | `Range` (single window aligned to chunk offsets), `dag-scope: block`, `X-SoraFS-Chunker`, optional `X-SoraFS-Nonce`, and mandatory base64 `X-SoraFS-Stream-Token`. |
| **Responses** | `206` with `Content-Type: application/vnd.ipld.car`, `Content-Range` describing the served window, `X-Sora-Chunk-Range` metadata, and echoed chunker/token headers. |
| **Failure modes** | `416` for misaligned ranges, `401` for missing/invalid tokens, `429` when stream/byte budgets are exceeded. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Single-chunk fetch with the same headers plus the deterministic chunk digest.
Useful for retries or forensic downloads when CAR slices are unnecessary.

## Multi-Source Orchestrator Workflow

When SF-6 multi-source fetch is enabled (Rust CLI via `sorafs_fetch`,
SDKs via `sorafs_orchestrator`):

1. **Collect inputs** — decode the manifest chunk plan, pull the latest adverts,
   and optionally pass a telemetry snapshot (`--telemetry-json` or
   `TelemetrySnapshot`).
2. **Build a scoreboard** — `Orchestrator::build_scoreboard` evaluates
   eligibility and records rejection reasons; `sorafs_fetch --scoreboard-out`
   persists the JSON.
3. **Schedule chunks** — `fetch_with_scoreboard` (or `--plan`) enforces range
   constraints, stream budgets, retry/peer caps (`--retry-budget`,
   `--max-peers`), and emits a manifest-scoped stream token for each request.
4. **Verify receipts** — outputs include `chunk_receipts` and
   `provider_reports`; CLI summaries persist `provider_reports`,
   `chunk_receipts`, and `ineligible_providers` for evidence bundles.

Common errors raised to operators/SDKs:

| Error | Description |
|-------|-------------|
| `no providers were supplied` | No eligible entries after filtering. |
| `no compatible providers available for chunk {index}` | Range or budget mismatch for a specific chunk. |
| `retry budget exhausted after {attempts}` | Increase `--retry-budget` or evict failing peers. |
| `no healthy providers remaining` | All providers disabled after repeated failures. |
| `streaming observer failed` | Downstream CAR writer aborted. |
| `orchestrator invariant violated` | Capture manifest, scoreboard, telemetry snapshot, and CLI JSON for triage. |

## Telemetry & Evidence

- Metrics emitted by the orchestrator:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (tagged by manifest/region/provider). Set `telemetry_region` in config or via
  CLI flags so dashboards partition by fleet.
- CLI/SDK fetch summaries include persisted scoreboard JSON, chunk receipts,
  and provider reports which must ship in rollout bundles for SF-6/SF-7 gates.
- Gateway handlers expose `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  so SRE dashboards can correlate orchestrator decisions with server behaviour.

## CLI & REST Helpers

- `iroha app sorafs pin list|show`, `alias list`, and `replication list` wrap the
  pin-registry REST endpoints and print raw Norito JSON with attestation blocks
  for audit evidence.
- `iroha app sorafs storage pin` and `torii /v1/sorafs/pin/register` accept Norito
  or JSON manifests plus optional alias proofs and successors; malformed proofs
  raise `400`, stale proofs surface `503` with `Warning: 110`, and
  hard-expired proofs return `412`.
- `iroha app sorafs repair list` mirrors repair queue filters, while
  `repair claim|complete|fail|escalate` submit signed worker actions or slash
  proposals to Torii. Slash proposals may include a governance approval summary
  (approve/reject/abstain vote counts plus approved_at/finalized_at
  timestamps); when present it must satisfy quorum and dispute/appeal windows,
  otherwise the proposal stays in dispute until votes resolve at the deadline.
- Repair listings and worker queue selection are ordered by SLA deadline, failure severity, and provider backlog with deterministic tie-breakers (queued time, manifest digest, ticket id).
- Repair status responses include an `events` array containing base64 Norito
  `RepairTaskEventV1` entries ordered by occurrence for audit trails; the list
  is capped to the most recent transitions.
- `iroha app sorafs gc inspect|dry-run --data-dir=/var/lib/sorafs` emits read-only
  retention reports from the local manifest store for audit evidence.
- REST endpoints (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) include attestation structures so clients can
  verify data against the latest block headers before taking action.

## References

- Canonical spec:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito types: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI helpers: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orchestrator crate: `crates/sorafs_orchestrator`
- Dashboard pack: `dashboards/grafana/sorafs_fetch_observability.json`
