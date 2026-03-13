---
lang: zh-hant
direction: ltr
source: docs/source/zk/lifecycle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bbb52acd8905ddd06241487decefaba4cfb2e5e8ae70c67b3a5f5cd185f221f6
source_last_modified: "2026-01-22T14:45:02.077723+00:00"
translation_last_reviewed: 2026-02-07
---

# Verifying-Key & Proof Lifecycle

This document captures how verifying keys (VKs) and zero-knowledge proof envelopes flow through Iroha v2. The intent is to give operators and SDK authors a single reference that ties together on-chain state, Torii app endpoints, and the CLI helpers.

## High-level flow

1. **Authoritative VK creation** – An operator or compiler artefact produces a verifying key and its 32-byte commitment. VKs are versioned and namespaced by backend (`backend::name`).
2. **Admission through Torii** – Operators post signed governance instructions to Torii (`/v2/zk/vk/*`). Accepted transactions register or update the VK registry that is stored on-chain and replicated across peers.
3. **Contract/runtime usage** – Transactions and smart contracts reference VKs by `(backend, name)` or embed an inline VK payload. Execution resolves VK commitments during proof verification.
4. **Proof verification** – Clients submit proofs (`/v2/zk/verify` or `/v2/zk/submit-proof`). Verification runs inside `iroha_core::zk` during transaction execution. Successful verifications materialise `ProofRecord`s that can be queried via Torii (`/v2/zk/proofs*`).
5. **Background reporting** – The optional Torii prover worker (`torii.zk_prover_enabled=true`) scans attachments, verifies `ProofAttachment` payloads, and exports telemetry describing proof sizes and processing latency. Reports are deleted automatically after the configured TTL.

## Verifying keys

- Registry entries live in the world state under `verifying_keys[(backend, name)]`.
- When a new VK is registered, the commitment must match the hashed payload or inline bytes bundled with the transaction. Updates bump the version and must preserve monotonicity.

### Relevant endpoints

- `POST /v2/zk/vk/register` – submit a signed `RegisterVerifyingKey` instruction.
- `POST /v2/zk/vk/update` – submit `UpdateVerifyingKey` with a higher version.
- `GET  /v2/zk/vk` – list VKs with optional filters (`backend`, `status`, `name_contains`).
- `GET  /v2/zk/vk/{backend}/{name}` – fetch a single VK record.

### CLI helpers

`iroha_cli app zk` exposes thin wrappers that post the JSON DTOs Torii expects:

- `iroha_cli app zk vk register --json path/to/register.json`
- `iroha_cli app zk vk update --json path/to/update.json`
- `iroha_cli app zk vk deprecate --json path/to/deprecate.json`
- `iroha_cli app zk vk get --backend halo2/ipa --name vk_transfer`

The JSON DTOs mirror the `iroha_data_model::proof` payloads. Inline VK bytes remain base64-encoded, while commitments are lowercase hex strings.

## Proof lifecycle

### Submission & verification

- Proof envelopes are accepted via `/v2/zk/verify` (synchronous) or `/v2/zk/submit-proof` (for later inspection). Both accept either Norito-encoded envelopes or JSON DTOs.
- During transaction execution, `iroha_core::smartcontracts::isi::zk::VerifyProof` hashes the proof bytes together with the backend name, derives a `ProofId`, and ensures the proof is unique across the ledger.
- The verifier resolves VK commitments from either the inline payload, the referenced `(backend, name)` pair, or both. Backends registered under `debug/*` bypass cryptographic checks for development.
- The resulting `ProofRecord` stores:
  - `backend` and `proof_hash`
  - `status` (`Submitted`, `Verified`, `Rejected`)
  - `verified_at_height` (block height when verification finished)
  - Optional `vk_ref` and `vk_commitment`
- ZK1/TLV envelopes are inspected at verification time. Recognised 4-byte tags are recorded lazily to power tag-based queries.

### Query surface

`/v2/zk/proofs` and `/v2/zk/proofs/count` expose the ledger-facing records:

- Filters: `backend`, `status`, `has_tag`, `offset`, `limit`, `order=asc|desc`, `ids_only`.
- Tag filtering is efficient: tags are indexed at verification time and served from a dedicated `(tag → proof ids)` index.
- `ids_only=true` returns `{ backend, hash }` objects for lightweight pagination.
- `/v2/zk/proof/{backend}/{hash}` remains available for direct lookups.

### CLI coverage

New subcommands are available under `iroha_cli app zk proofs`:

- `iroha_cli app zk proofs list [--backend halo2/ipa] [--status Verified] [--has-tag PROF] [--limit 20]`
- `iroha_cli app zk proofs count [--backend halo2/ipa] [--has-tag IPAK]`
- `iroha_cli app zk proofs get --backend halo2/ipa --hash 0123...`

All commands emit Norito JSON responses. Filters match the HTTP query parameters one-to-one, making it easy to script pagination or supply the output into monitoring tooling.

## Background prover & telemetry

- Controlled via `torii.zk_prover_enabled`, `torii.zk_prover_scan_period_secs`, `torii.zk_prover_reports_ttl_secs`, `torii.zk_prover_max_inflight`, `torii.zk_prover_max_scan_bytes`, `torii.zk_prover_max_scan_millis`, `torii.zk_prover_keys_dir`, `torii.zk_prover_allowed_backends`, and `torii.zk_prover_allowed_circuits` in `iroha_config`.
- Attachments must decode as `ProofAttachment`/`ProofAttachmentList` (Norito or JSON). ZK1/TLV envelopes are tagged but rejected as top‑level payloads.
- Backends are allowlisted by prefix; default `["halo2/"]`. `stark/fri-v1/*` is supported when built with feature `zk-stark` and enabled via config (`zk.stark.enabled=true`). `groth16/…` remains unsupported.
- Each report now records `latency_ms = processed_ms - created_ms` so operators can track queue delays.
- The prover emits telemetry:
  - `torii_zk_prover_attachment_bytes` (histogram, labelled by `content_type`)
  - `torii_zk_prover_latency_ms` (histogram)
  - `torii_zk_prover_inflight` (gauge) and `torii_zk_prover_pending` (gauge)
  - `torii_zk_prover_last_scan_bytes` and `torii_zk_prover_last_scan_ms` (gauges)
  - `torii_zk_prover_budget_exhausted_total{reason}` (counter)
  - `zk_verify_latency_ms` and `zk_verify_proof_bytes` (histograms, labelled by `backend`)
- Metrics surface under `/metrics` when telemetry is enabled with a profile that allows metrics exposure.
- Reports older than the TTL are garbage-collected on every scan tick. Manual deletions remain available through `/v2/zk/prover/reports`.

Nightly Milestone 0 runs scrape the new histograms and publish rollups alongside the existing Torii operator dashboard, ensuring proof verification latency regressions surface quickly.
