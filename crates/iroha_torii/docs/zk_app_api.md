# Torii App API: ZK Attachments and Prover (Operator Guide)

This document describes the app‑facing ZK endpoints exposed by Torii for storing proof attachments and inspecting background prover reports. These features are strictly non‑consensus and do not affect block production or validation.

Contents
- Endpoints and storage layout
- Configuration (TTL, size caps, tokens, rate limits)
- Operational notes and safety
- Examples (curl and CLI)

## Endpoints

Attachments (store sanitized bytes with metadata):
- `POST /v1/zk/attachments` — store a new attachment (body = bytes, sanitized on ingest)
- `GET  /v1/zk/attachments` — list stored attachments (JSON array)
- `GET  /v1/zk/attachments/{id}` — fetch stored attachment bytes by id
- `DELETE /v1/zk/attachments/{id}` — delete an attachment

Background prover reports (non‑consensus verification):
- `GET    /v1/zk/prover/reports` — list reports (JSON array)
- `GET    /v1/zk/prover/reports/{id}` — fetch one report (JSON)
- `DELETE /v1/zk/prover/reports/{id}` — delete a report

IVM prove helper (non-consensus proof generation):
- `POST   /v1/zk/ivm/derive` — execute IVM bytecode and derive an `IvmProved` payload (commitments only)
- `POST   /v1/zk/ivm/prove` — submit a prove job for an `IvmProved` payload (returns `{ job_id }`)
- `GET    /v1/zk/ivm/prove/{job_id}` — poll job status (`pending|running|done|error`)
- `DELETE /v1/zk/ivm/prove/{job_id}` — delete/cancel a job from the in-memory cache

Notes
- Attachment id is a deterministic Blake2b‑32 (hex, lowercase) of the sanitized body bytes.
- Content type is normalized from magic‑byte sniffing; the declared header is recorded in `provenance`.
- Attachments persist on disk under `./storage/torii/zk_attachments/`:
  - `./storage/torii/zk_attachments/{id}.bin` (body)
  - `./storage/torii/zk_attachments/{id}.json` (metadata)
- Prover reports persist under `./storage/torii/zk_prover/reports/{id}.json`.
- Base directory is configured with `torii.data_dir`; tests/dev harnesses can override with `data_dir::OverrideGuard`.
- IVM derive/prove require bytecode with the IVM ZK mode bit set (`mode & ZK != 0`) and request metadata that includes `gas_limit`.

## Configuration

All runtime behavior is configured via `iroha_config` (Torii section). The following keys are relevant:

- `torii.attachments_ttl_secs` (u64)
  - Time‑to‑live for attachments. A background GC removes entries older than this TTL.
  - Default: 7 days.
- `torii.attachments_max_bytes` (u64)
  - Maximum size (bytes) for a single attachment. Requests over the cap receive `413 Payload Too Large`.
  - Default: 4 MiB.
- `torii.attachments_per_tenant_max_count` (u64)
  - Maximum number of attachments retained per tenant (0 disables the cap). Tenants are derived from `x-api-token` (hashed as `token:<blake2b32>`); missing tokens fall back to `anon`.
  - Default: 128.
- `torii.attachments_per_tenant_max_bytes` (u64)
  - Aggregate bytes retained per tenant. When uploads would exceed the cap, Torii deterministically evicts the oldest attachments for that tenant before persisting the new body. Bodies larger than the cap are rejected with `413`.
  - Default: 64 MiB.
- `torii.attachments_allowed_mime_types` (list of strings)
  - Allowlisted MIME types for attachment payloads after magic‑byte sniffing.
  - Default: `["application/x-norito", "application/json", "text/json", "application/x-zk1"]`.
- `torii.attachments_max_expanded_bytes` (u64)
  - Maximum expanded size for gzip/zstd payloads.
  - Default: 16 MiB.
- `torii.attachments_max_archive_depth` (u32)
  - Maximum nested gzip/zstd layers.
  - Default: 2.
- `torii.attachments_sanitizer_mode` (string)
  - Sanitizer execution mode (`subprocess` or `in_process`).
  - Default: `subprocess`.
- `torii.attachments_sanitize_timeout_ms` (u64)
  - Sanitization timeout in milliseconds.
  - Default: 1000.
- `torii.zk_prover_enabled` (bool)
  - Enables background prover scan worker. When disabled, endpoints still serve existing reports, but no new reports are created.
  - Default: false.
- `torii.zk_prover_scan_period_secs` (u64)
  - Scan period for the background prover.
  - Default: 30 seconds.
- `torii.zk_prover_reports_ttl_secs` (u64)
  - Retention window for prover reports.
- `torii.zk_prover_max_inflight` / `torii.zk_prover_max_scan_bytes` / `torii.zk_prover_max_scan_millis`
  - Concurrency and per-scan budgets for the prover worker.
- `torii.zk_prover_keys_dir` (path)
  - Directory holding verifying key bytes for registry entries without inline keys.
- `torii.zk_prover_allowed_backends` / `torii.zk_prover_allowed_circuits` (string list)
  - Allowlists for prover scope (prefix match, empty = allow all).
- `torii.zk_ivm_prove_max_inflight` / `torii.zk_ivm_prove_max_queue`
  - Concurrency controls for the IVM prove helper endpoint (`POST /v1/zk/ivm/prove`).
  - When saturated, Torii rejects new jobs with `429` and a `Retry-After` hint.
- `torii.zk_ivm_prove_job_ttl_secs` / `torii.zk_ivm_prove_job_max_entries`
  - Retention controls for the in-memory prove job cache used by `/v1/zk/ivm/prove/{job_id}`.
  - Jobs older than `zk_ivm_prove_job_ttl_secs` are evicted (pending/running jobs are cancelled best-effort to free capacity).
- `torii.max_content_len` (bytes)
  - Global HTTP request body limit; applies to attachments uploads as an upper bound.

Access control and rate limiting:
- `torii.require_api_token` (bool) and `torii.api_tokens` (list of strings)
  - When enabled and tokens are provided, app API routes require `x-api-token: <token>` header. Unknown/missing tokens are rejected.
- `torii.query_rate_per_authority_per_sec` (Option<u32>) and `torii.query_burst_per_authority` (Option<u32>)
  - Per-authority rate limiter for app API endpoints (attachments and prover routes).
- `torii.tx_rate_per_authority_per_sec` (Option<u32>) and `torii.tx_burst_per_authority` (Option<u32>)
  - Per-authority transaction submission rate limiter for `/v1/transaction` and other tx-producing endpoints.
- `torii.api_allow_cidrs` (list of CIDRs)
  - Requests from these networks bypass the rate limiter (still subject to body size limits and tokens, if enabled).

Tip: These keys map to the `iroha_config::parameters::user::Torii` section and are threaded into Torii at startup. Environment variables exist for developer convenience (e.g., `TORII_ATTACHMENTS_TTL_SECS`), but prefer static config files in production.

## Operational Notes

- Non‑consensus: attaching proofs and reading prover reports does not modify WSV or affect consensus. They are app/operator tools.
- Determinism & safety: id derivation and report generation are deterministic (based on body and content type). The prover verifies proofs using configured backends; it does not generate proofs or affect consensus.
- Sanitization: gzip/zstd payloads are expanded within configured limits and only allowlisted types are stored; sanitizer metadata is captured in `provenance` and exports are re‑sanitized.
- GC cadence: attachment GC runs every minute and removes entries older than `attachments_ttl_secs`.
- Storage hygiene: deleting an attachment removes both `.bin` and `.json`; deleting a report removes the corresponding `.json` under `zk_prover/reports`.
- Payloads: the prover expects `ProofAttachment`/`ProofAttachmentList` payloads (Norito or JSON). ZK1/TLV envelopes are tagged but rejected as top‑level payloads.
- Key bytes: when a registry entry omits inline VK bytes, the prover loads bytes from `torii.zk_prover_keys_dir` using `<backend>__<name>.vk` naming.
- Proving keys: the IVM prove helper (`/v1/zk/ivm/prove`) loads proving key bytes from the same directory using `<backend>__<name>.pk` naming.
  The `.pk` file must match the resolved verifying key and uses Halo2 `SerdeFormat::Processed` serialization.
- Privacy: neither `/v1/zk/ivm/derive` nor `/v1/zk/ivm/prove` expose plaintext gas usage (`gas_used`). Gas usage is committed inside `gas_policy_commitment`.
- Execution semantics: the current `halo2/ipa:ivm-execution-v1` circuit used by `/v1/zk/ivm/prove` is a commitment-binding circuit and does not prove full IVM execution semantics by itself. Nodes still deterministically replay the bytecode during admission to recompute the overlay and commitments and reject mismatches.
- Metrics: `torii_zk_ivm_prove_inflight` (jobs currently proving) and `torii_zk_ivm_prove_queued` (jobs queued waiting for an inflight slot) expose IVM prove helper queue pressure.

## Examples

Using curl:

```bash
# Upload a JSON proof envelope (ensure Content-Type is set)
curl -sS -X POST \
  -H 'Content-Type: application/json' \
  --data-binary @proof.json \
  http://localhost:8080/api/v1/zk/attachments | jq .

# List attachments
curl -sS http://localhost:8080/api/v1/zk/attachments | jq .

# Download attachment bytes
curl -sS http://localhost:8080/api/v1/zk/attachments/<id> -o downloaded.bin

# List background prover reports (if enabled)
curl -sS http://localhost:8080/api/v1/zk/prover/reports | jq .

# Fetch a single report
curl -sS http://localhost:8080/api/v1/zk/prover/reports/<id> | jq .
```

With tokens/rate limits:

```bash
# When require_api_token=true and tokens are configured
curl -sS -H 'x-api-token: my-token' http://localhost:8080/api/v1/zk/attachments
```

CLI shortcuts (`iroha_cli`):

```bash
# Upload attachment
iroha app zk attachments upload --file ./proof.json --content-type application/json

# List/get/delete
iroha app zk attachments list
iroha app zk attachments get --id <id> --out ./downloaded.bin
iroha app zk attachments delete --id <id>

# Prover reports (list/get/delete)
iroha app zk prover reports list
iroha app zk prover reports get --id <id>
iroha app zk prover reports delete --id <id>

# IVM prove helper (submit/poll) and proving-key derivation
iroha app zk ivm prove --json ./ivm_prove_request.json --wait
iroha app zk ivm get --job-id <job_id>
iroha app zk ivm delete --job-id <job_id>
iroha app zk ivm derive-pk --vk ./halo2_ipa__ivm-exec-v1.vk --out ./halo2_ipa__ivm-exec-v1.pk
```

See also: the ZK vote tally convenience endpoint (`POST /v1/zk/vote/tally`) and CLI helper `iroha app zk vote tally` for inspecting election tallies.
