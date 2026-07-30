## SoraFS Node ↔ Client Protocol (SF-2)

This document captures the initial protocol surface for discovering and
connecting to SoraFS storage providers. Provider advertisements are Norito
payloads signed by governed operators and distributed over the discovery mesh.
Clients consume these adverts to build fetch routes, enforce path-diversity
controls, and decide when to refresh or evict providers.

### Provider Advertisements

Provider advertisements use the `ProviderAdvertV1` layout defined in
`crates/sorafs_manifest::provider_advert`. Each advert carries a signed
envelope, a bounded TTL, and routing metadata that guides client selection. Providers
aliases via `profile_aliases`, enabling discovery while clients transition.
Unknown capability TLVs should be ignored when the advert sets
`allow_unknown_capabilities`, following the GREASE guidance below.

```text
ProviderAdvertV1 {
    version        : u8                // must equal 1
    issued_at      : u64 (unix secs)
    expires_at     : u64 (unix secs)
    body           : ProviderAdvertBodyV1
    signature      : AdvertSignature
    signature_strict : bool            // true = signature MUST verify
    allow_unknown_capabilities : bool // true = clients may ignore unknown capability codes
}
```

The advertisement **must** validate the following constraints:

- `expires_at > issued_at`
- `issued_at ≤ now` (future-issued adverts are rejected)
- `expires_at - issued_at ≤ 86 400 seconds (24 h)`
- `capabilities`, `endpoints`, and `rendezvous_topics` are non-empty
- `path_policy.min_guard_weight > 0`
- `qos.max_retrieval_latency_ms > 0` and `qos.max_concurrent_streams > 0`

The body enumerates stake information, QoS targets, and the capability TLVs
that callers rely on to negotiate transports.

```text
ProviderAdvertBodyV1 {
    provider_id        : [u8; 32]    // governance-controlled ID
    profile_id         : string      // chunker / storage profile
    stake              : StakePointer
    qos                : QosHints
    capabilities       : [CapabilityTlv]
    endpoints          : [AdvertEndpoint]
    rendezvous_topics  : [RendezvousTopic]
    path_policy        : PathDiversityPolicy
    notes              : option<string>
    stream_budget      : option<StreamBudgetV1>
    transport_hints    : option<[TransportHintV1]>
}
```

| Field | Description |
|-------|-------------|
| `provider_id` | BLAKE3-256 digest assigned by governance. Used for stake and admission checks. |
| `profile_id`  | Declares the canonical chunking/profile handle (e.g., `sorafs.sf1@1.0.0`). |
| `profile_aliases` | Optional list of alternate handles appended to `profile_id` so clients can advertise them in `Accept-Chunker`. The canonical handle MUST appear in the list when the field is present. |
| `stake`       | Points to the staking pool and amount governing admission. Zero stake fails validation. |
| `qos`         | Encodes availability tier, maximum retrieval latency, and concurrent stream capacity. |
| `capabilities` | TLV list describing transport features. Reserved codes cover Torii, QUIC+Noise, and SoraNet PQ; additional payload allows vendor extensions. |
| `endpoints`   | Concrete service endpoints. Each entry includes the endpoint kind, host pattern, and metadata (TLS fingerprint, ALPN, region). |
| `rendezvous_topics` | Topics advertised in the DHT so clients can discover providers by profile and region. |
| `path_policy` | Guard rails that constrain path construction (minimum guard weight, maximum providers from the same ASN or staking pool). |
| `stream_budget` | Optional concurrency and throughput envelope (`StreamBudgetV1`). Clients clamp in-flight requests to `max_in_flight` and treat `max(max_bytes_per_sec, burst)` as a hard per-chunk ceiling. Only valid when the advert also includes `CapabilityType::ChunkRangeFetch`. |
| `transport_hints` | Ordered list of supported ranged-fetch transports (`TransportHintV1`). Priorities are low numbers first and align with CLI `--transport-hint` entries. Hints must be non-empty when present and require the `chunk_range_fetch` capability. SoraNet transport hints (`soranet_relay`) additionally require a SoraNet capability flag (`soranet_pq`). |
| `signature_strict` | Must be `true` for Torii discovery ingestion. Signature verification is mandatory and remote data cannot relax it. |
| `allow_unknown_capabilities` | Signals that callers MAY retain the advert even if it contains capability codes they do not yet recognise. Clients drop the unknown TLVs while honouring the advertised capability policy for the known entries. |

QoS hints expose deterministic expectations for schedulers:

- `availability` selects one of the tiered latency classes: `Hot` (sub-second),
  `Warm` (under one minute cold-start), or `Cold` (archival lanes with relaxed
  SLAs). Providers MUST publish telemetry to ensure the advertised tier matches
  measured latency.
- `max_retrieval_latency_ms` is the provider's P95 single-chunk retrieval bound.
  Values of zero are rejected; operators SHOULD keep the bound within the chosen
  availability tier (e.g., `Hot` providers typically advertise ≤1 500 ms).
- `max_concurrent_streams` declares the deterministic concurrency budget
  available to clients. Schedulers clamp active fetches per provider to this
  value.

The signature covers `sorafs.provider-advert.v1\0` followed by the canonical
Norito encoding of the advert envelope: version, issuance and expiry timestamps,
body, signature algorithm and public key, `signature_strict`, and
`allow_unknown_capabilities`. Signature bytes themselves are excluded. This
prevents copied signatures from changing TTL or policy. Ed25519 is the only
accepted first-release algorithm; the multi-signature enum value remains
reserved.

For operator tooling, the repository ships `sorafs_provider_advert`. Production
signing is private-key-free and uses a two-phase external Ed25519/HSM handoff.
The first phase writes the exact canonical signing payload; after the external
signer returns a raw 64-byte signature, the second phase verifies the reviewed
payload, signer key, SHA-256 key fingerprint, and signature before emitting the
Norito advert:

```bash
advert_args=(
  --chunker-profile=sorafs.sf1@1.0.0 \
  --provider-id=001122... \
  --stake-pool-id=ffeedd... \
  --stake-amount=5 \
  --availability=hot \
  --max-latency-ms=1500 \
  --max-streams=4 \
  --capability=torii \
  --capability=quic \
  --soranet-pq=strict \
  --range-capability=max_span=1048576,min_granularity=4096,sparse=true,alignment=false,merkle=true \
  --stream-budget=max_in_flight=4,max_bytes_per_sec=5000000,burst=2000000 \
  --transport-hint=torii:0 \
  --transport-hint=quic:1 \
  --transport-hint=soranet:2 \
  --endpoint=torii:storage.example.com \
  --endpoint-meta=region:global \
  --topic=sorafs.sf1.primary:global \
  --issued-at=1700000000
)

cargo run -p sorafs_car --bin sorafs_provider_advert -- \
  --prepare "${advert_args[@]}" \
  --public-key-file=provider.pub \
  --public-key-fingerprint-sha256="$REVIEWED_PROVIDER_KEY_SHA256" \
  --signing-payload-out=provider-advert.signing-payload \
  --json-out=provider-advert.signing-request.json

# Send provider-advert.signing-payload to the governed PKCS#11/HSM signer.
# The signer returns exactly 64 raw Ed25519 signature bytes in provider.sig.

cargo run -p sorafs_car --bin sorafs_provider_advert -- \
  --emit "${advert_args[@]}" \
  --public-key-file=provider.pub \
  --public-key-fingerprint-sha256="$REVIEWED_PROVIDER_KEY_SHA256" \
  --signing-payload-file=provider-advert.signing-payload \
  --signature-file=provider.sig \
  --advert-out=provider.advert \
  --json-out=provider.report.json

```

The first-release CLI is a hard cut. `--chunker-profile` accepts only the exact
`namespace.name@semver` handle; numeric IDs, registry aliases, slash-form
handles, case variants, and whitespace-padded values fail. Selector tokens and
structured field names are likewise exact and case-sensitive: availability is
`hot|warm|cold`; generic capability selectors are
`torii|quic|potr-mldsa|vendor`; SoraNet PQ and range support use
`--soranet-pq=guard|majority|strict` and `--range-capability`, respectively;
booleans are only `true|false`; endpoint kinds are
`torii|quic|norito-rpc`; and endpoint metadata keys are
`tls_fingerprint|alpn|region`. Compatibility spellings are rejected instead of
being normalized.

Provider, stake-pool, capability-payload, endpoint-metadata, and reviewed
fingerprint hex must be lowercase, even-length, prefix-free, and
whitespace-free. Fixed-width identifiers such as `--provider-id` and
`--stake-pool-id` must be exactly 32 bytes (64 hex characters); the tool does
not left-pad short values before signing. Public-key, signing-payload, signature,
and advert inputs must be bounded direct regular files with stable identity:
symlinks, hard links, mutable group/world permissions, path replacement, and
wrong-length raw key/signature material fail closed.

`--range-capability` emits the `CapabilityType::ChunkRangeFetch` TLV with a
structured payload (`ProviderCapabilityRangeV1`). Providers declare the largest
chunk span (`max_span`), the minimum seek granularity (`min_granularity`), and
optional flags for sparse offsets, alignment, and Merkle proof support. The CLI
rejects missing or inconsistent values and ensures `min_granularity ≤ max_span`.
The accepted field set is exactly
`max_span|min_granularity|sparse|alignment|merkle`; each field may occur at most
once.

`--soranet-pq` emits the typed `SoraNetHybridPq` capability. Each value names
one cumulative posture: `guard`, `majority`, or `strict`. Stage aliases and
multi-token combinations are not accepted.

`--stream-budget` introduces the optional `StreamBudgetV1` block that accompanies
range-capable providers. It declares a deterministic concurrency ceiling
(`max_in_flight`), a sustained throughput reservation (`max_bytes_per_sec`), and
an optional burst allowance (`burst`, bytes). When present, clients clamp in-flight
requests to the advertised `max_in_flight` and treat `max(burst, max_bytes_per_sec)`
as the upper bound for single-chunk transfers. Both `--stream-budget` and
`--transport-hint` require `--range-capability`; the CLI enforces this so
non-range providers cannot advertise budgets or transports they cannot honour.
Norito-level validators perform the same check and also reject empty
`transport_hints` arrays to prevent malformed offline adverts.

`--transport-hint=protocol:priority` may be repeated to describe the preferred
transport ordering for ranged fetches. CLI selectors map to canonical report/API
labels as follows: `torii` → `torii_http_range`, `quic` → `quic_stream`,
`soranet` → `soranet_relay`, and `vendor` → `vendor_reserved`. These output
labels are not accepted as CLI aliases. Hints are only accepted when a range
capability is present so downgraded providers cannot advertise transports they
cannot serve. Client stubs now reject range-capable adverts that omit either
`stream_budget` or `transport_hints`, preventing malformed metadata from
entering the multi-source scoreboard or scheduler.

Capability selectors follow the same input/output distinction: `torii`, `quic`,
the dedicated range flag, the dedicated SoraNet PQ flag, `potr-mldsa`, and
`vendor` emit the canonical labels `torii_gateway`, `quic_noise`,
`chunk_range_fetch`, `soranet_pq`, `potr_mldsa`, and `vendor_reserved`.

During multi-source retrieval the orchestrator verifies each chunk against the
advertised metadata before scheduling it. A provider is skipped (and, if no
eligible peers remain, retrieval fails with `NoCompatibleProviders`) when:

- The chunk length exceeds `max_chunk_span`.
- Alignment is required but the chunk offset or length is not a multiple of
  `min_granularity`.
- A stream budget exists and the chunk length surpasses the burst/rate ceiling.

This fast-fail path prevents deterministic plans from stalling on providers that
would have rejected the request mid-flight.

Operators should refresh adverts at least every 12 hours (or sooner if
capabilities change) and push them through the discovery mesh.

### Chunk Range Retrieval (Gateway API)

Gateways expose deterministic HTTP endpoints that honour the advert metadata
described above. Clients must present the same chunker handle and range windows
they negotiated via discovery; the gateway rejects misaligned requests before
streaming any payload bytes.

### Multi-Source Fetch Flow

Multi-source retrieval orchestrates the gateway APIs above while preserving the
deterministic ordering described by the manifest chunk plan. Each session
performs the following steps:

1. **Discovery & eligibility.** The client fetches `ProviderAdvertV1` entries
   via the discovery mesh, verifying signatures and capability TLVs. Providers
   missing the required `chunk_range_fetch` capability, alignment support, or
   fresh telemetry are marked ineligible and omitted from scheduling.
2. **Scoreboard construction.** The client builds a scoreboard using manifest
   metadata, provider adverts, and optional telemetry snapshots. Eligible
   providers receive normalised weights and concurrency budgets derived from
   their stream tokens and `max_concurrent_streams` hints. When persisted, the
   scoreboard JSON records every ineligible provider with the rejection reason.
3. **Stream-token allocation.** For each provider, the client obtains a
   manifest-scoped stream token and embeds it in subsequent HTTP requests via
   `X-SoraFS-Stream-Token`. Tokens encode per-provider byte and concurrency
   budgets; the orchestrator refuses to schedule a chunk if the budget would be
   exceeded.
4. **Chunk scheduling.** The orchestrator selects a provider for each chunk
   using weighted round-robin scheduling. Selection honours alignment and burst
   limits before dispatching `GET /v1/sorafs/storage/car/{manifest_id}` (for
   multi-chunk ranges) or `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`
   (for individual chunks). Responses are verified against the manifest digest
   and chunk length before being assembled.
5. **Failure handling.** When a provider fails a request, the orchestrator
   increments its consecutive failure counter and retries the chunk with the
   next eligible provider until the per-chunk retry budget (default `3`) is
   exhausted. Providers that exceed `provider_failure_threshold` are disabled
   for the remainder of the session. If all providers become disabled, the
   client surfaces `MultiSourceError::NoHealthyProviders` to the caller.

During a session the orchestrator emits structured telemetry:

- `telemetry::sorafs.fetch.lifecycle` — start/complete events (chunk count,
  duration, retries).
- `telemetry::sorafs.fetch.retry` — retry attempts per provider.
- `telemetry::sorafs.fetch.provider_failure` — disabled providers and reasons.
- `telemetry::sorafs.fetch.error` — terminal failures (no providers, retry
  budget exhausted, observer failures).

Clients should capture the JSON fetch summary returned by the CLI or SDK,
persisting `provider_reports`, `chunk_receipts`, and `ineligible_providers`.
These artefacts form the audit trail for staged rollouts and blacklisting.

#### `GET /v1/sorafs/storage/car/{manifest_id}`

Fetches a CAR slice aligned to full chunk boundaries. Required headers:

| Header | Notes |
|--------|-------|
| `Range` | Single byte range (`bytes=start-end`). `start`/`end` must align to chunk offsets. |
| `dag-scope` | Must equal `block`. Other values are rejected. |
| `X-SoraFS-Chunker` | Canonical chunker handle (e.g., `sorafs.sf1@1.0.0`). Must match the stored manifest. |
| `X-SoraFS-Nonce` | Caller-chosen nonce echoed back in the response. |
| `X-SoraFS-Stream-Token` | Required base64-encoded Norito stream token authorising the request. |

Responses include:

- Status `206 Partial Content` with `Content-Type: application/vnd.ipld.car`.
- `Content-Range: bytes start-end/total_bytes` describing the satisfied window.
- `X-Sora-Chunk-Range: start={start};end={end};chunks={count}` with chunk count
  covering the range.
- `X-SoraFS-Chunker` echoing the manifest’s chunker handle.
- Echoed `X-SoraFS-Stream-Token` / `X-SoraFS-Nonce`.

If the requested window is misaligned or extends past the stored payload, the
gateway responds `416 Range Not Satisfiable` with `Content-Range: bytes */total`.

Missing or invalid stream tokens yield `401 Unauthorized`. Tokens that exceed
their stream or byte budgets return `429` with an explanatory error payload so
clients can back off before retrying.

### Multi-Source Fetch Flow

Clients that enable the SF-6 orchestrator use the advert metadata above plus runtime telemetry to assemble deterministic multi-peer fetches:

1. **Collect inputs** — decode the manifest’s chunk plan, gather the latest adverts for the desired providers, and snapshot runtime telemetry (`TelemetrySnapshot`). CLI operators can provide the snapshot via `--telemetry-json` while SDKs pass `TelemetrySnapshot` structures directly to `sorafs_orchestrator`.
2. **Build the scoreboard** — call `sorafs_orchestrator::Orchestrator::build_scoreboard` (or `sorafs_fetch --scoreboard-out`) to evaluate eligibility. Ineligible entries include a `reason` (expired advert, stale telemetry, capability mismatch). Persisted scoreboard JSON documents the weights applied in subsequent steps.
3. **Schedule chunks** — invoke `Orchestrator::fetch_with_scoreboard` (or `sorafs_fetch --plan`) to stream chunks in parallel. The orchestrator enforces range/alignment constraints, stream budgets, per-session failure guards, and optional caps on the number of peers selected (`max_providers`/`--max-peers`).
4. **Verify receipts** — the fetch outcome embeds `chunk_receipts` (provider + attempt count per chunk) and `provider_reports` (success/failure totals). Dashboards use these fields to highlight chronic offenders and elevated retry windows.

When the scheduler rejects a candidate, the CLI reports the same variant of `MultiSourceError` that the SDK surfaces:

| Error | Meaning |
|-------|---------|
| `no providers were supplied` | The manifest plan was invoked without any providers or eligible scoreboard entries. |
| `no compatible providers available for chunk {index}` | All candidates violated range capability or stream budget requirements for the chunk. |
| `retry budget exhausted after {attempts}` | The per-chunk retry limit was hit without a successful response. Increase the retry budget or remove unstable peers. |
| `no healthy providers remaining after {attempts}` | Providers exist but all were disabled after repeated failures. |
| `streaming observer failed` | Downstream consumers (e.g., CAR writers) aborted the fetch. |
| `orchestrator invariant violated` | Internal bug; capture the persisted scoreboard, telemetry snapshot, and CLI JSON outputs for triage. |

Telemetry emitted alongside each session records:

- `sorafs_orchestrator_active_fetches{manifest_id,region}`
- `sorafs_orchestrator_fetch_duration_ms{manifest_id,region}`
- `sorafs_orchestrator_retries_total{manifest_id,provider}`
- `sorafs_orchestrator_provider_failures_total{manifest_id,provider}`

Set `telemetry_region` on the orchestrator configuration (or `--telemetry-region` when exposed via wrappers) to keep dashboards partitioned by fleet. Persisted scoreboard artefacts should accompany change requests and incident reports so operators can replay the session deterministically.

#### `GET /v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}`

Returns a single chunk (BLAKE3-256 digest) for the given manifest. Required
headers:

| Header | Notes |
|--------|-------|
| `X-SoraFS-Nonce` | Required nonce echoed for auditing. |
| `X-SoraFS-Chunker` | Optional but, when supplied, must match the stored manifest. |
| `X-SoraFS-Stream-Token` | Required base64-encoded stream token; echoed in the response. |

Responses include:

- Status `200 OK` with `Content-Type: application/octet-stream`.
- `X-Sora-Chunk-Range: start={offset};end={offset+len-1};chunks=1`.
- `X-SoraFS-Chunk-Digest: {chunk_digest}`.
- Echoed `X-SoraFS-Nonce` and `X-SoraFS-Stream-Token`.

The gateway rejects missing or expired stream tokens with `401 Unauthorized`
and enforces per-token byte budgets with `429` responses.

Chunks are served only when the manifest advertises the
`chunk_range_fetch` capability and the requested digest exists on disk. Requests
for unknown digests map to `404` via the underlying storage error surface.

When the orchestrator retries a chunk after an HTTP error, it records the
failure under the originating provider. Providers that repeatedly trigger `401`,
`429`, or `5xx` responses accumulate penalties and may be disabled once the
session threshold is reached.

### Alias Resolution & CLI helpers

The Torii REST surface exposes alias discovery under `/v1/sorafs/alias`. The
Iroha CLI bundles thin wrappers so operators can validate bindings without
manually crafting HTTP requests:

```bash
# Resolve an alias to its manifest/account owner.
iroha alias resolve --alias sora/docs

# Resolve the alias at Merkle index 42.
iroha alias resolve-index --index 42

# Validate the alias format locally without contacting Torii.
iroha alias resolve --alias sora/docs --dry-run
```

The commands forward to the Torii endpoints and stream any JSON payloads back
to stdout. Successful `resolve` calls print the manifest/account identifier and
the source (`manifest`, `replication`, etc.) when Torii includes it in the
response. When Torii responds with `404` the CLI prints a friendly “not found”
message instead of a stack trace.

The SoraFS subcommands under `iroha app sorafs …` expose structured JSON for pin
registry listings (`pin list`), alias enumeration (`alias list`), replication
order listings (`replication list`), finalized repair tasks (`repair list`), and
storage stats (`storage show`). Pin/alias/replication list commands accept
pagination parameters (`--limit`, `--offset`) and filters (for example
`--alias-namespace`, `--manifest-digest`). `repair list` instead uses a bounded
immutable task-id cursor, an optional exact finalized height/block-hash pair,
and optional `--ticket-id` for the singular finalized task. GC helpers (`gc
inspect`, `gc dry-run`) scan the local storage directory to report retention
deadlines and expired manifests without performing deletions.

Effective retention is computed as the minimum of `pin_policy.retention_epoch`
and any manifest metadata caps (`sorafs.retention.deal_end_epoch`,
`sorafs.retention.governance_cap_epoch`). Missing or zero-valued caps are
treated as unbounded. The GC JSON output includes `retention_sources` so
operators can see which constraint set each manifest’s expiry. When storage is
under capacity pressure, GC sweeps evict expired manifests by least-recently-used
ordering with `manifest_id` tie-breakers.

### Pin Registry REST Endpoints

Torii now exposes read-only views of the on-chain pin registry so operators
and SDKs can inspect manifests without issuing ad-hoc queries:

- `GET /v1/sorafs/pin` returns the paginated manifest catalogue alongside an
  `attestation` block hash. Optional query parameters:
  - `status=pending|approved|retired`
  - `limit` (defaults to 50, capped at 500)
  - `offset`
- `GET /v1/sorafs/pin/{digest_hex}` returns exact native Norito JSON
  `PinManifestFinalizedRecordV1`: `finalized_cursor` contains the finalized
  height and block-hash bytes, while `manifest` is the chain-authoritative
  `PinManifestRecord`, including `por_root`, `content_length`, and `status`.
  Callers may supply `expected_finalized_height` and
  `expected_finalized_block_hash_hex` together to pin the read; an unmatched
  cursor returns HTTP 409. Malformed requests return HTTP 400, missing manifests
  return HTTP 404, and unavailable authoritative state returns HTTP 503. The
  response is `Cache-Control: no-store`; the retired `limit`, attestation,
  alias-list, replication-order, cache-state, and proof-expiry projections are
  neither accepted nor returned. Query aliases and replication orders through
  their dedicated list endpoints.
- `POST /v1/sorafs/pin/register` accepts the closed JSON V1 request. Its required
  `manifest_payload` is exact canonical padded base64 of canonical Norito
  `ManifestV1` bytes. Torii derives the digest, chunker, content length, pin
  policy, and fee inputs solely from that decoded manifest and rejects retired
  duplicate summary fields. `alias` and `successor_of_hex` remain optional; a
  supplied predecessor must be a nonzero canonical digest. The alias proof must
  be valid base64, and unknown, unapproved, or retired predecessors return HTTP
  400 so cycles never enter the registry.
- `GET /v1/sorafs/aliases` lists active alias bindings with filters for
  `namespace` and `manifest_digest`, sharing the same `limit`/`offset` controls
  as the manifest listing.
- `GET /v1/sorafs/replication` surfaces governance-issued replication orders.
  Clients can scope the response via `status=pending|completed|expired` and
  `manifest_digest` query parameters.

Registry list responses retain their listing attestation. The single-manifest
read instead binds the native record directly to its `finalized_cursor`.

### CLI Helpers

The `iroha` CLI wraps the REST endpoints for day-to-day operations:

- `iroha app sorafs pin list --status=approved` returns the registry snapshot and
  its `attestation` metadata so operators can verify the reported block hash.
- `iroha app sorafs pin show --digest=<hex>` fetches a single manifest together with
  bound aliases and replication orders.
- `iroha app sorafs alias list --namespace=docs` and
  `iroha app sorafs replication list --status=pending` mirror the REST filters.
- `iroha app sorafs repair list` reads the finalized task page, or one task with
  `--ticket-id`; callers can pin a scan with
  `--expected-finalized-height` and `--expected-finalized-block-hash` and resume
  with `--after-task-id`.
- `repair claim|renew|complete|fail|escalate` constructs exactly one matching
  native instruction, signs the containing Iroha transaction with the CLI
  authority, and submits the exact bytes to the route-specific strict durable
  ingress. Each command returns a transaction hash after the route accepts it;
  the caller must reconcile the finalized task/event projection before acting
  on the transition.
- Repair commands carry the expected task revision; renew, complete, fail, and
  escalate also carry the exact live lease generation. The native instruction
  enforces provider-scoped authority, lease ownership/expiry, idempotency, and
  terminal uniqueness.
- At the HTTP boundary, report, slash, appeal, claim, heartbeat, complete, and
  fail command routes return `202 Accepted`; finalized status, task-list,
  single-task, and committed-event queries return `200 OK`.
- `iroha app sorafs storage prepare --manifest=manifest.to --payload=payload.bin
  --payload-out=provider.payload --files-out=provider.files.json` prepares
  deterministic material for the provider-internal worker. It does not submit
  bytes over HTTP or reserve provider capacity.
- `iroha app sorafs gc inspect|dry-run --data-dir=/var/lib/sorafs` emits read-only
  retention reports from the local manifest store for audit evidence.
- GC eviction sweeps emit `GcAuditEventV1` payloads into the governance DAG, so
  operators can archive retention evidence alongside repair and settlement
  artefacts.

Every command prints the raw Norito JSON response, which makes it trivial to
feed into scripting or capture in attestation logs.


### Storage Scheduler State

Operators and monitoring agents can query `/v1/sorafs/storage/state` to
retrieve the latest scheduler snapshot exposed by Torii. The response mirrors
`StorageStateResponseDto` and includes queue depth, fetch throughput, PoR worker
utilisation, and the raw bytes-per-second counters wired from
`StorageSchedulersRuntime`.【crates/iroha_torii/src/sorafs/api.rs:260】

These gauges power the `torii_sorafs_storage_*` Prometheus metrics described in
the node plan and let dashboards visualise pin back-pressure and PoR activity
without polling the storage backend directly.【crates/iroha_torii/src/routing.rs:5143】
Torii also publishes high-level registry and client access metrics that back
the SLO work in SF-7:

- `torii_sorafs_chunk_range_requests_total{endpoint,status}` and
  `torii_sorafs_chunk_range_bytes_total{endpoint}` drive request/byte rate SLOs
  for range-capable gateways.【crates/iroha_telemetry/src/metrics.rs:4303】
- `torii_sorafs_registry_*` gauges expose manifest counts, alias inventory, and
  replication order backlog so dashboards can track governance health without
  scraping JSON endpoints.【crates/iroha_telemetry/src/metrics.rs:5278】
- `torii_sorafs_gc_*` counters/gauges surface retention sweeps, bytes freed,
  blocked evictions, and expired-manifest age for capacity dashboards.
- `torii_sorafs_alias_cache_refresh_total{result,reason}` and
  `torii_sorafs_alias_cache_age_seconds` capture the alias cache evaluation
  path surfaced above, allowing operators to alert on stale/expired proofs.

### Storage Round-trip Flow

The end-to-end storage tests follow a deterministic sequence to confirm the API
and telemetry stay in sync.【crates/iroha_torii/tests/sorafs_discovery.rs:989-1150】

1. Register the manifest on-chain with `RegisterPinManifest`; the submitter pays
   the configured SoraFS public pin fee and the approved registry record stores
   the manifest digest, chunk digest, content length, policy, fee asset, treasury,
   and fee amount. That fee metadata is a committed receipt; storage ingest does
   not reprice the record against later governance schedule or treasury changes.
2. After the registration is finalized, the supervised provider worker
   reconciles the exact finalized height/hash, approved manifest record,
   configured provider identity, and committed replication assignment. It
   derives a durable idempotency key from those committed fields before
   ingesting provider-supplied bytes. V1 exposes no public storage-upload route;
   HTTP requests cannot change storage/quota state or provider-keyed metadata.
3. `POST /v1/sorafs/storage/fetch` using the returned manifest id, an offset,
   and a bounded length. The response echoes the request fields and streams the
   chunk data as `data_b64`.
4. Submit an authenticated `POST /v1/sorafs/proof/stream` PoR request bound to
   the canonical manifest digest and provider identifier. The request must carry
   the non-zero `expected_finalized_height` and canonical
   `expected_finalized_block_hash_hex` returned beside the authoritative record
   in step one; Torii rejects an incomplete or stale cursor and streams only
   witnesses that verify against that record's committed root. The
   unauthenticated local storage PoR route is retired.
5. `GET /v1/sorafs/storage/state` to verify the scheduler snapshot. A fully
   successful cycle drives `pin_queue_depth`, `fetch_inflight`, and
   `por_inflight` back to zero, bumps `fetch_bytes_per_sec` above zero (smoothing
   accounts for elapsed time), and increments `por_samples_success_total` by the
   sampled leaf count while leaving `por_samples_failed_total` untouched.

Failed provider-internal delivery remains in the bounded durable outbox and
moves to a payload-free dead letter after the governed attempt limit. Restart
replays the same idempotency identity; a caller cannot select a different
provider or overwrite another provider's metadata.

This workflow doubles as the CLI recipe for validating stream budgets: any
fetch that exceeds a provider’s advertised burst or alignment policy will fail
before reaching step four, ensuring operators can reproduce the test locally.

### Stream Token Issuance

Range-enabled gateways mint signed stream tokens through
`POST /v1/sorafs/storage/token`. Clients must supply canonical headers and
the canonical request payload:

- Header `X-API-Token`: exactly one configured Torii API credential. Stream
  token issuance requires this credential even when the deployment-wide
  `torii.require_api_token` switch is false. A missing or invalid credential is
  rejected with HTTP `401`; enabling issuance without at least one canonical
  `torii.api_tokens` credential is rejected at node startup. An inconsistent
  runtime state still fails closed with HTTP `503`.
- Header `X-SoraFS-Client`: a 1–128 byte visible-ASCII diagnostic label echoed
  to the caller. It is not authentication and is not a quota key. Missing,
  whitespace, control-character, and oversized values are rejected with HTTP
  `400`.
- Header `X-SoraFS-Nonce`: a 1–128 byte visible-ASCII value echoed verbatim in
  the response. Missing, whitespace, control-character, and oversized values
  are rejected with HTTP `400`.
- Body (`StreamTokenRequestDto`): `manifest_id_hex`, `provider_id_hex` (32-byte
  BLAKE3 digest), and optional overrides (`ttl_secs`, `max_streams`,
  `rate_limit_bytes`, `requests_per_minute`). Hex is canonical lowercase with
  no surrounding whitespace. The provider must be non-zero and exactly match
  the gateway's declared provider identity. Every override must be positive and
  no greater than its configured default; request values cannot raise a
  gateway's ceiling.

When issuance is enabled in node TOML and startup has bound the configured
runtime signer, the handler responds with:

- HTTP `200 OK`.
- Headers:
  - `X-SoraFS-Nonce` — the caller-provided nonce.
  - `X-SoraFS-Client` — the echoed client identifier.
  - `X-SoraFS-Verifying-Key` — lowercase hex-encoded Ed25519 verifying key that
    signed the token. Clients cache this key to verify `signature_hex`.
  - `X-SoraFS-Token-Id` — the canonical identifier for the issued token body.
  - `X-SoraFS-Issuance-Quota-Remaining` — remaining issuance requests in the
    current 60 s window for the authenticated API credential, not the echoed
    client label. The gateway stores only a domain-separated credential digest
    as the quota subject, so changing `X-SoraFS-Client` cannot reset the
    allowance. Zero/unlimited issuance policies are rejected. When the quota
    is exhausted the gateway returns `429` together with `Retry-After`
    indicating when the window resets and
    `X-SoraFS-Issuance-Quota-Remaining: 0`.
  - `Cache-Control: no-store` — responses are never cacheable.
- Body:

```json
{
  "token": {
    "body": {
      "token_id": "0123456789abcdef0123456789abcdef",
      "manifest_cid_hex": "<lowercase-hex-manifest-cid>",
      "provider_id_hex": "<64-lowercase-hex-provider-id>",
      "profile_handle": "sorafs.sf1@1.0.0",
      "max_streams": 4,
      "ttl_epoch": 1731638400,
      "rate_limit_bytes": 5242880,
      "issued_at": 1731634800,
      "requests_per_minute": 120,
      "token_pk_version": 7
    },
    "signature_hex": "4f06…"
  }
}
```

`body` mirrors `StreamTokenBodyV1` encoded via Norito so downstream services can
verify the signature deterministically. `signature_hex` is the detached
Ed25519 signature over the domain separator
`sorafs.stream-token.signature.v1\0` followed by
`body.to_canonical_bytes()`. The transport form is canonical padded base64 and
is bounded to 4096 header bytes. Token IDs are exactly 16 random bytes rendered
as 32 lowercase hexadecimal characters. A token has positive concurrency,
request, and byte budgets, a maximum one-hour lifetime, and no more than 60
seconds of positive issuance-clock skew. It is rejected at its exact expiry
second. Nodes return HTTP `404` with
`{"error": "stream token issuance is not enabled on this node"}` only when
issuance is disabled in node TOML. Enabled production startup fails closed
unless a runtime-injected HSM/KMS signer reports the configured non-secret
`signer_handle` and exact Ed25519 `signer_public_key_hex`. No signing-seed file,
key path, or environment enablement is accepted; signer credentials and private
key material remain runtime-only.

Authenticated-credential issuance and per-token quota state are bounded and
prune only expired or idle windows. A full state table never evicts an active
budget (which would reset its quota); the gateway instead returns HTTP `503`
with `Retry-After: 1`. Clock rollback and poisoned accounting state also fail
closed with `503`.

### Chunk Range Fetch RPC

Once a manifest is pinned, clients retrieve deterministic byte ranges via
`POST /v1/sorafs/storage/fetch`. The request body mirrors the DTO defined in
Torii (`StorageFetchRequestDto`) and must include the manifest digest, the byte
offset, and the number of bytes to return. The serving provider identity comes
from the node's governed runtime configuration and cannot be supplied or
overridden by the caller.【crates/iroha_torii/src/sorafs/api.rs:68】

```json
{
  "manifest_id_hex": "4e548e7d6336c8f47f31d4615a6ad3424c5b421ec8d2157d5c8c1f963602f9cc",
  "offset": 0,
  "length": 65536
}
```

Nodes respond with a base64-encoded payload (`StorageFetchResponseDto`) and echo
the offset and length so higher layers can verify alignment and track
progress.【crates/iroha_torii/src/sorafs/api.rs:83】

Request parameters must honour the provider’s advertisement:

- `length` cannot exceed `max_chunk_span` for the selected provider, and the
  response is rejected with `NoCompatibleProviders` if every provider would
  violate the limit.【crates/sorafs_car/src/multi_fetch.rs:456-520】
- When `requires_alignment` is true, both `offset` and `length` must align to
  `min_granularity`; misaligned requests fail fast and bubble up the alignment
  reason to the caller.【crates/sorafs_car/src/multi_fetch.rs:456-520】
- If a `StreamBudgetV1` is advertised, the orchestrator clamps parallelism and
  rejects chunks whose length exceeds the permitted burst window to prevent
  overloads.【crates/sorafs_car/src/multi_fetch.rs:642-783】

The repository ships a deterministic multi-source plan in
`fixtures/sorafs_manifest/provider_admission/multi_fetch_plan.json` that SDKs
can reuse to validate offset/length handling against the default chunker. The
fixture is a strict `sorafs.chunk_fetch_plan.v1` object whose
`payload_digest_blake3_hex` binds the complete fixture payload; SDKs must reject
the retired bare-array form and missing or substituted bindings. Tests also
exercise downgrade paths and ensure the cache surfaces the correct warnings for
providers missing range capabilities.【crates/iroha_torii/tests/sorafs_discovery.rs:333-392】【crates/iroha_torii/tests/sorafs_discovery.rs:410-435】

When orchestrating multi-source retrieval, clients should iterate over the plan
and submit fetch requests in `max_chunk_span`-bounded slices. The CLI’s
end-to-end tests cover this flow and expose the resulting provider receipts so
integrations can assert deterministic scheduling.【crates/sorafs_car/src/multi_fetch.rs:1341-1501】

For audits, run
`sorafs_provider_advert --verify --advert=<path> --public-key-file=<raw-key-path>
--public-key-fingerprint-sha256=<reviewed-hex> [--now=unix_ts]`. Verification
requires the advert signer to equal the exact reviewed raw public key, validates
the fingerprint and Ed25519 signature, enforces TTL/path/QoS rules, and prints a
JSON summary (optionally via `--json-out`). The JSON payload includes
`signature_verified=true` only when the signature matches the complete canonical
envelope and explicitly reports `signature_strict`. Torii only ingests adverts
where this field is `true`.

### TTL, Refresh, and Expiry

Advertisements can remain valid for **at most 24 hours**. Clients refresh
their provider sets half-way through the window, clamping the refresh deadline
to the 12 hour recommendation so long-lived adverts refresh deterministically:

- `ProviderAdvertV1::ttl()` returns the advertised TTL.
- `ProviderAdvertV1::refresh_deadline()` = `issued_at + min(⌈TTL/2⌉, 12 h)`.

When the TTL is shorter than 12 hours the half-way mark is used verbatim. For
example, a 3-second TTL refreshes 2 seconds after issuance, while the default
24-hour TTL refreshes after 12 hours. If an advert is still valid but no
refresh has arrived by the computed deadline, the path manager treats the
provider as **stale** and rotates it out of active circuits until a fresh advert
is seen.

### Capability GREASE

When it is `true`, providers signal that callers MAY retain the advert even if it
contains capability TLVs whose `cap_type` values are unknown to that client.
Such clients should drop the unrecognised entries, honour the remaining known
capability set, and surface telemetry so operators can detect the encounter.
When the flag is `false`, unknown capability codes MUST cause validation failure
to keep the discovery mesh aligned with the ratified capability list.

Providers are encouraged to periodically publish deterministic
`CapabilityType::VendorReserved` entries while `allow_unknown_capabilities` is
set. Doing so keeps older clients tolerant of new capability numbers and helps
future protocol revisions roll out without being blocked by stale deployments.

### Rendezvous Topics

Each advert publishes one or more `RendezvousTopic` entries. Topics follow the
pattern `sorafs.<profile>.<lane>` (e.g., `sorafs.sf1.primary`). The `region`
field accepts ISO‑3166 alpha‑2 codes or `global`. Clients use the topic +
region pair to perform deterministic routing and to mix providers across
geographies when the path policy allows it.

### Chunker Negotiation

Providers implicitly advertise the active chunker profile via the registry
metadata embedded in manifests and via dedicated HTTP headers during data
transfer. Negotiation follows HTTP content negotiation:

1. **Request.** Clients send an `Accept-Chunker` header listing supported
   `(namespace, name, semver)` tuples in descending preference order. Each list
   element follows the ABNF
   ```
   chunker-value = namespace "." name ";version=" semver [";q=" qvalue]
   namespace     = 1*ALPHA ; ASCII letters only
   name          = 1*(ALPHA / DIGIT / "-" / "_")
   semver        = 1*(ALPHA / DIGIT / "." / "-" / "+")
   ```
   where `qvalue` mirrors the HTTP `Accept` weighting (default `1.0`). Clients
   MUST include the canonical handle `sorafs.sf1@1.0.0` unless they intentionally
   deprecate SF1.
2. **Selection.** Gateway chooses the highest-preference profile it can serve.
   When multiple values share the same `qvalue`, servers prefer the lowest
   semantic version that satisfies the request to keep output deterministic.
   If no overlap exists, the gateway responds with `406 Not Acceptable` and
3. **Response.** Successful responses MUST include `Content-Chunker` with the
   selected canonical handle (e.g., `sorafs.sf1@1.0.0`). When the response is
   generated by a bridge lane (see below), the header additionally includes
4. **Manifests.** Manifests always record the chosen profile inside
   `ChunkingProfileV1`. Consumers treat the manifest as the source of truth and
   validate the HTTP headers against it; mismatches abort the retrieval.

Clients MAY additionally send `Accept-Digest` alongside `Accept-Chunker`. Gateways
SHA-256 digest for bridge responses. The chunker selection is evaluated first; a
digest mismatch yields `406 Not Acceptable` with both `Content-Chunker` and
`Content-Digest` headers describing the supported combination.

Unknown chunker handles are not fatal: if a manifest references a profile that
is missing from the local registry, clients fall back to the inline parameters
embedded in `ChunkingProfileV1`. However, providers MUST NOT advertise handles
outside the chartered namespace (`sorafs.*`) until the new profile is ratified.
`profile_aliases`; clients SHOULD mirror that union (including canonical) when
names deterministically.

### Anti-Eclipse & Path Diversity

The `PathDiversityPolicy` gives clients minimum constraints when constructing
retrieval circuits:

- `min_guard_weight`: the minimum stake percentile for guard selection.
- `max_same_asn_per_path`: maximum number of providers from the same ASN in a
  single circuit (typically 1 for guard nodes).
- `max_same_pool_per_path`: prevents multiple providers from the same staking
  pool occupying the path simultaneously.

Clients enforce these values in addition to their local policies. Lower values
are rejected outright; higher (stricter) values are permitted. This allows
governance to ratchet the defaults while still letting cautious clients
strengthen constraints.

### Admission & Sybil Controls

Only governed identities may publish provider adverts. Operators sign the envelope
with an Ed25519 key whose public key is registered in the governance registry.
Stake pointers reference the staking pool record in the ledger; Torii validates
the pointer before accepting or broadcasting an advert.

When an advert is signed by the CLI using a managed key, `signature_strict` is
set to `true`. Torii always verifies the signature first and rejects adverts that
request relaxed verification. For each provider, a replacement must also have a
strictly greater `issued_at`; exact byte-identical duplicates are idempotent, while
older replays and conflicting adverts with the same timestamp are rejected without
removing the current cache record. Before mutating the live cache, Torii atomically
persists the provider's issuance timestamp and advert fingerprint in the canonical
Norito checkpoint configured by `replay_checkpoint_path`. The checkpoint survives
restart and expiry/admission pruning, preventing a shorter-lived update from being
followed by replay of an older advert that is still within its own TTL. Exact copies
of the durable high-water advert may repopulate an empty post-restart cache; a
different advert at the same timestamp remains a downgrade and is rejected.

`replay_checkpoint_max_entries` bounds the checkpoint and must be at least the
number of identities in the admission registry; the default and hard first-release
maximum are both 65,536. Checkpoint entries are strictly sorted and unique, and
the file is read with bounded,
no-follow semantics and updated using a private atomic temporary file plus rename.
Torii retains an exclusive operating-system lock on the adjacent `.lock` file for
the cache lifetime; a second process pointed at the same checkpoint fails closed
instead of allowing last-writer-wins high-water regression.
An oversized, corrupt, non-canonical, permissively accessible, symlink-backed, or
admission-inconsistent checkpoint fails closed by disabling the discovery cache.
Operators must treat `reason="replay_checkpoint"` as a local integrity incident;
deleting the checkpoint to restore service reopens replay protection and is not an
acceptable recovery procedure.

If governance revokes a provider, its public key is removed from the registry
and future adverts fail signature validation.

### Client Behaviour Summary

1. Fetch new adverts from rendezvous topics bound to the desired profile.
2. Validate each advert via `ProviderAdvertV1::validate_with_body(now)` and
   confirm the signature against the governance registry.
3. Enforce refresh scheduling at `refresh_deadline()` while evicting expired
   providers immediately.
4. Construct circuits obeying the advertised path policy and local safeguards.
5. Grease capability negotiation by ignoring unknown capability codes while
   retaining the advert (payloads may be vendor-specific but do not break

The Rust implementation of the advertisement schema now ships with deterministic
encoding tests so downstream consumers (Go, TypeScript, Swift) can re-use the
Norito layout without reverse-engineering the protocol.
