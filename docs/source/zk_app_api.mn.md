---
lang: mn
direction: ltr
source: docs/source/zk_app_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2034856d110367bcecc37c1b28645de27168ec70d6c3275466855b466139f271
source_last_modified: "2026-01-22T16:26:46.597827+00:00"
translation_last_reviewed: 2026-02-07
---

# ZK App API: Attachments and Prover Reports (Operator Guide)

This document describes the app‑facing ZK endpoints exposed by Torii for handling proof attachments and background prover reports. These facilities are non‑consensus: they do not affect validation, execution, or block formation. They are intended for operator tooling and UI/UX flows.

Key properties:
- Deterministic, non‑forking behavior. Disabling the worker does not change consensus results.
- Feature‑gated under `app_api` (enabled by default for Torii).
- Rate‑limited and optionally protected by API tokens.
- On‑disk storage under `./storage/torii` by default.

## Attachments

Attachments store sanitized artifacts such as proof envelopes or JSON DTOs. Each attachment is addressed by a deterministic id derived from the sanitized bytes.

Endpoints:
- `POST /v2/zk/attachments` — store attachment, returns metadata `{ id, size, content_type, created_ms, provenance? }`.
- `GET  /v2/zk/attachments` — list metadata for stored attachments (JSON array).
- Supports filters via query params: `id`, `content_type` (substring), `since_ms`, `before_ms`, `has_tag=<TAG>` (ZK1 TLV tag present, e.g., `PROF`, `IPAK`), `limit`, `offset`, `order=asc|desc`, `ids_only=true`.
- `GET  /v2/zk/attachments/:id` — fetch stored attachment bytes by id; content type is preserved.
- `DELETE /v2/zk/attachments/:id` — delete stored attachment and metadata.
 - `GET  /v2/zk/attachments/count` — return `{ count }` for the same filter set as the list endpoint.
- `GET  /v2/zk/proof/{backend}/{hash}` — fetch a proof record by backend and proof hash (64‑hex). Returns JSON `{ backend, proof_hash, status, verified_at_height?, vk_ref?, vk_commitment? }`.
 - `GET  /v2/zk/proofs` — list proof records with optional filters and pagination.
   - Filters: `backend`, `status=Submitted|Verified|Rejected`, `has_tag=<TAG>` (requires `zk-proof-tags` feature), `verified_from_height`, `verified_until_height`, `limit`, `offset`, `order=asc|desc`, `ids_only=true`.
 - `GET  /v2/zk/proofs/count` — return `{ count }` for the same filter set.
- Proof endpoints enforce Torii’s dedicated guardrails:
  - Body limits: proof submission payloads exceeding `torii.proof_max_body_bytes` are rejected.
  - Rate limit: `torii.proof_rate_per_minute` + `torii.proof_burst` (returns `429` + `Retry-After` using `torii.proof_retry_after_secs` unless `api_allow_cidrs` bypasses).
  - Pagination: `torii.proof_max_list_limit` caps `limit`; larger requests fail with `CapacityLimit` (429).
  - Timeout: list/count handlers abort after `torii.proof_request_timeout_ms` wall-clock.
  - Egress throttling: proof fetches are shaped by `torii.proof_egress_bytes_per_sec` + `torii.proof_egress_burst_bytes`; throttled responses return `Retry-After` with the proof retry hint.
  - Caching: `GET /v2/zk/proof/...` emits `Cache-Control: public,max-age=torii.proof_cache_max_age_secs` and `ETag="<proof_hash>"`; `If-None-Match` returns `304 Not Modified` without body.
  - Metrics: `torii_proof_requests_total`, `torii_proof_request_duration_seconds`, `torii_proof_response_bytes_total`, `torii_proof_cache_hits_total`, and `torii_proof_throttled_total` expose outcomes and cache hits per endpoint.

Details:
- Deterministic id: Blake2b‑32 of the sanitized request body bytes (lower‑case hex).
- Content‑Type: normalized to the sniffed type (magic‑byte inspection). The declared header is recorded in `provenance.declared_type`.
- Provenance: responses include `provenance` with `{ declared_type, sniffed_type, hashes { blake2b_256, sha256 }, sanitizer { verdict, expanded_bytes, archive_depth, sandboxed } }`.
- Rejections: unsupported types return `415 Unsupported Media Type`; expansion/sandbox failures return `413`/`400` with the rejection reason in the body.
- Size cap: enforced per item via `torii.attachments_max_bytes` (default 4 MiB). Requests exceeding the cap receive `413 Payload Too Large`.
- Per-tenant quota: Torii enforces per-tenant attachment limits using `torii.attachments_per_tenant_max_count` (count) and `torii.attachments_per_tenant_max_bytes` (aggregate bytes). Tenants are derived from `X-API-Token` (hashed as `token:<blake2b-32 hex>`); requests without a token fall back to tenant `anon`. If an upload would exceed either limit, Torii deterministically evicts the oldest attachments for that tenant before persisting the new body. When the incoming body alone exceeds `torii.attachments_per_tenant_max_bytes`, Torii returns `413 Payload Too Large`.
- Retention (TTL): attachments older than `torii.attachments_ttl_secs` (default 7 days) are removed by a background GC that runs approximately every 60 seconds.
- Storage layout:
  - Data: `storage/torii/zk_attachments/<id>.bin`
  - Metadata: `storage/torii/zk_attachments/<id>.json`

## Background Prover Reports

The background prover worker (disabled by default) scans attachments and produces a JSON report per attachment. It verifies `ProofAttachment` payloads (single or list) using the core ZK backend verifiers:

- Norito (`application/x-norito`): the body must decode as `ProofAttachment` or `ProofAttachmentList`.
- JSON (`application/json`, `text/json`): the body must decode as a `ProofAttachment` object, a `ProofAttachmentList` (base64 string), or a JSON array of `ProofAttachment`.
- ZK1/TLV envelopes are not accepted as top‑level attachment payloads; they are tagged (`zk1_tags`) but reported as `ok=false`.

Verification rules:
- `vk_ref` is resolved via the WSV verifying‑key registry. When a registry entry omits inline key bytes, Torii loads the key bytes from `torii.zk_prover_keys_dir` (see storage layout below).
- `vk_commitment` is validated against the computed VK hash when present.
- Backends and circuits are allowlisted via `torii.zk_prover_allowed_backends` and `torii.zk_prover_allowed_circuits` (prefix match).
- Supported backends currently include `halo2/ipa` and other `halo2/…` variants built into the node. `stark/fri-v1/*` is supported when built with feature `zk-stark` and enabled via config (`zk.stark.enabled=true`). `groth16/…` remains unsupported.

Endpoints:
- `GET /v2/zk/prover/reports` — list reports as a JSON array.
  - Supports optional query parameters for server-side filtering:
    - `ok_only=true|false` and `failed_only=true|false` (mutually exclusive)
    - `errors_only=true` (alias for `failed_only=true`)
    - `id=<hex>` (exact match)
    - `content_type=<substring>` (substring match)
    - `has_tag=<TAG>` (ZK1 TLV tag present, e.g., `PROF`, `IPAK`)
    - `errors_only=true` (alias for `failed_only=true`)
    - `limit=<N>` (cap results; max 1000)
    - `since_ms=<u64>` (return only reports with `processed_ms >= since_ms`)
    - `before_ms=<u64>` (return only reports with `processed_ms <= before_ms`)
    - `order=asc|desc` (default `asc`), `offset=<u32>` (apply after ordering and filtering)
    - `latest=true` (return only the most recent report after filters)
    - `ids_only=true` (return only ids as an array of strings)
    - `messages_only=true` (return only `{ id, error }` objects for failed reports)
- `GET /v2/zk/prover/reports/:id` — fetch a single report.
- `DELETE /v2/zk/prover/reports` — bulk delete reports matching filters (same query parameters as list). Returns `{ deleted, ids }`.
- `DELETE /v2/zk/prover/reports/:id` — delete a report by id.

Report schema (JSON):
```json
{
  "id": "<64-hex>",
  "ok": true,
  "error": null,
  "content_type": "application/json",
  "size": 123,
  "created_ms": 1710000000000,
  "processed_ms": 1710000300000,
  "backend": "halo2/ipa",
  "vk_ref": { "backend": "halo2/ipa", "name": "vk_main" },
  "proof_hash": "…",
  "circuit_id": "halo2/ipa:tiny-add-v1"
}
```

When an attachment carries multiple proofs, the report includes a `proofs` array with per‑proof `{ backend, ok, error, vk_ref, proof_hash, circuit_id }` entries.

Storage layout:
- Reports: `storage/torii/zk_prover/reports/<id>.json`
- Prover key store: `storage/torii/zk_prover/keys/<backend>__<name>.vk` (sanitized components; override via `torii.zk_prover_keys_dir`).

Retention:
- Attachments are subject to TTL GC (see above).
- Prover reports are also subject to TTL GC controlled by `torii.zk_prover_reports_ttl_secs` (default: 7 days). The prover worker deletes reports older than the TTL on each scan cycle. Operators may still delete reports explicitly via the API.

## Configuration

Configure via `iroha_config` (`[torii]` section). Defaults are shown in comments.

TOML (recommended for production):
```toml
[torii]
# Attachments
attachments_ttl_secs = 604800         # 7 days
attachments_max_bytes = 4_194_304     # 4 MiB
attachments_per_tenant_max_count = 128
attachments_per_tenant_max_bytes = 8_388_608   # 8 MiB aggregate per tenant
attachments_allowed_mime_types = ["application/x-norito", "application/json", "text/json", "application/x-zk1"]
attachments_max_expanded_bytes = 16_777_216    # 16 MiB expanded payload cap
attachments_max_archive_depth = 2              # max nested gzip/zstd layers
attachments_sanitize_timeout_ms = 1000         # sanitizer timeout (ms)
attachments_sanitizer_mode = "subprocess"      # subprocess or in_process

# Background prover (non-consensus)
zk_prover_enabled = false             # disabled by default
zk_prover_scan_period_secs = 30       # scan every 30 seconds when enabled
zk_prover_reports_ttl_secs = 604800   # delete reports older than 7 days
zk_prover_max_inflight = 2            # process up to 2 attachments concurrently
zk_prover_max_scan_bytes = 16_777_216 # cap each scan to 16 MiB of attachment data
zk_prover_max_scan_millis = 2000      # bail out after 2 seconds of wall-clock time per scan
zk_prover_keys_dir = "./storage/torii/zk_prover/keys"
zk_prover_allowed_backends = ["halo2/"] # prefix match (empty = allow all)
zk_prover_allowed_circuits = []       # prefix match (empty = allow all)

# (optional) app API tokens and rate limits
require_api_token = false
api_tokens = ["example-token-value"]
# Proof endpoint guardrails
proof_rate_per_minute = 120           # steady-state tokens/min (None to disable rate limiting)
proof_burst = 60                      # burst tokens per endpoint key
proof_max_body_bytes = 8_388_608      # maximum submission payload size (bytes)
proof_max_list_limit = 200            # maximum allowed `limit` for proofs list
proof_request_timeout_ms = 1000       # wall-clock timeout for list/count
proof_cache_max_age_secs = 30         # Cache-Control max-age for proof fetches
proof_retry_after_secs = 1            # Retry-After value returned on throttling
proof_egress_bytes_per_sec = 8_388_608 # optional steady-state egress budget (bytes/sec)
proof_egress_burst_bytes = 16_777_216 # optional egress burst budget (bytes)
```

Configuration must be set via `iroha_config` files. Environment variable overrides exist for developer tooling but are not intended for operator-facing deployments.

When the worker exhausts the byte or time budget, it stops scheduling new attachments, increments `torii_zk_prover_budget_exhausted_total{reason="bytes|time"}`, and leaves the remainder queued for the next scan. Live gauges expose the current workload via `torii_zk_prover_inflight` (attachments in progress), `torii_zk_prover_pending` (attachments yet to be scheduled), and the most recent cycle statistics: `torii_zk_prover_last_scan_bytes` and `torii_zk_prover_last_scan_ms`.

### Operations & Dashboards

- A full operator runbook covering alerting, log pivots, and incident
  procedures lives in [`zk/prover_runbook.md`](zk/prover_runbook.md).
- Import [`grafana_zk_prover.json`](grafana_zk_prover.json) into Grafana to seed
  dashboards for queue depth, attachment throughput, worker latency, and ledger
  correlation panels.
- Recommended alerts:
  - Page when `avg_over_time(torii_zk_prover_pending[10m]) > 0` or when
    `histogram_quantile(0.95, sum(rate(torii_zk_prover_latency_ms_bucket[5m])) by (le))`
    stays above your configured `zk_prover_max_scan_millis` for longer than
    15 minutes.
  - Ticket when `increase(torii_zk_prover_budget_exhausted_total{reason="bytes"}[30m]) > 0`
    so operators can adjust the byte budget or prune problematic attachments.

## Security and Operations

- The app API is subject to rate limits; limits are enforced per endpoint key. You can require an API token for app-facing endpoints via `require_api_token=true` and set `api_tokens`.
- CIDR allowlist (`api_allow_cidrs`) can bypass rate limits for trusted origins.
- These endpoints do not change consensus outcomes. You can disable the prover worker (`zk_prover_enabled=false`) without impacting validation or execution.

## CLI Helpers

Use the CLI to interact with the app API (requires Torii URL and any API token if configured):

- Attachments:
  - `iroha app zk attachments upload --file <PATH> [--content-type <MIME>]`
  - `iroha app zk attachments list`
  - `iroha app zk attachments get --id <ID> --out <PATH>`
  - `iroha app zk attachments delete --id <ID>`
- Verification support:
  - `iroha app zk verify --json <PATH>` or `--norito <PATH>`
  - `iroha app zk submit-proof --json <PATH>` or `--norito <PATH>`

## Verifying Key Registry (App API)

Torii exposes convenience endpoints to manage the on‑chain Verifying Key registry by submitting signed transactions or reading records. These endpoints are app‑facing shims; they build and submit ISIs on your behalf.

Endpoints:
- `POST /v2/zk/vk/register` — Submit `RegisterVerifyingKey`
- `POST /v2/zk/vk/update` — Submit `UpdateVerifyingKey` (version must increase)
- `POST /v2/zk/vk/deprecate` — Submit `DeprecateVerifyingKey`
- `GET  /v2/zk/vk/{backend}/{name}` — Get a verifying key record as JSON

`GET` responses normalise the data to:

```json5
{
  "id": { "backend": "halo2/ipa", "name": "vk_main" },
  "record": {
    "version": 3,
    "circuit_id": "halo2/ipa::transfer_v3",
    "backend": "halo2-ipa-pasta",
    "curve": "pallas",
    "public_inputs_schema_hash": "…",
    "commitment": "…",
    "vk_len": 40960,
    "max_proof_bytes": 8192,
    "gas_schedule_id": "halo2_default",
    "metadata_uri_cid": "ipfs://…",
    "vk_bytes_cid": "ipfs://…",
    "activation_height": 1200,
    "deprecation_height": null,
    "withdraw_height": null,
    "status": "Active",
    "key": { "backend": "halo2/ipa", "bytes_b64": "..." }
  }
}
```

When `ids_only=true`, the list endpoint returns objects containing just `{ "backend": "...", "name": "..." }`.

DTOs (POST bodies, JSON):
  - `vk_bytes` (base64) — full verifying key bytes; Torii computes the commitment and validates it against `commitment_hex` when present. `vk_len` is optional here but, when provided, must match the byte length.
  - `commitment_hex` (hex, 64) — commitment only; when bytes are omitted you must supply `vk_len` so the record captures verifier size metadata.

CLI wrappers:
- Register: `iroha app zk vk register --json ./vk_register.json`
- Update: `iroha app zk vk update --json ./vk_update.json`
- Deprecate: `iroha app zk vk deprecate --json ./vk_deprecate.json`
- Get: `iroha app zk vk get --backend <backend> --name <name>`

Example `vk_register.json`:
```json
{
  "authority": "i105...",
  "private_key": "ed0120...",
  "backend": "halo2/ipa",
  "name": "vk_main",
  "version": 1,
  "gas_schedule_id": "halo2_default",
  "max_proof_bytes": 8192,
  "metadata_uri_cid": "ipfs://CID_FOR_METADATA",
  "vk_bytes_cid": "ipfs://CID_FOR_VK_BUNDLE",
  "vk_bytes": "BASE64_BYTES",
  "vk_len": 40960
}
```

Notes:
- Commitments are domain‑separated hashes over `backend || bytes` and are verified on submit.

### Subscribing to Verifying Key Registry Events


Examples (JSON5):

1) Listen to all verifying key events for a specific id (backend + name):


2) Listen only for updates (version bumps) regardless of id:


CLI usage example:



### Subscribing to Proof Events

You can also subscribe to proof verification events via `DataEventFilter.Proof`. The CLI offers presets for convenience.

Examples:

1) Listen to all events for a proof id:

```
iroha ledger trigger register \
  --id proof_watch \
  --filter data \
  --data-proof halo2/ipa:0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd \
  --path ./on_proof.ko
```

2) Only successes (Verified) for that proof:

```
iroha ledger trigger register \
  --id proof_successes \
  --filter data \
  --data-proof halo2/ipa:0123abcd... \
  --data-proof-only verified \
  --path ./on_verified.ko
```

## Limitations and Future Work

- The prover worker verifies proofs using configured backend verifiers; it does not generate proofs (no proving pipeline).
- Report retention is controlled by `torii.zk_prover_reports_ttl_secs` and can be managed via the report delete endpoints.

## Governance Endpoints (ZK Ballots)

For submitting ZK ballots and building transaction skeletons, refer to the Governance App API document. Torii submits the ballot when `private_key` is provided; otherwise it returns a skeleton for clients to sign and submit:
- POST `/v2/gov/ballots/zk` — base DTO returning a `CastZkBallot` skeleton.
- POST `/v2/gov/ballots/zk-v1` — v1-style DTO with explicit envelope fields.
- POST `/v2/gov/ballots/zk-v1/ballot-proof` — accepts `BallotProof` JSON directly (feature `zk-ballot`).

See docs/source/governance_api.md for details and examples.
