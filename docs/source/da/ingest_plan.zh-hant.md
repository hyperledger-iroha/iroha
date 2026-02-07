---
lang: zh-hant
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
---

# Sora Nexus Data Availability Ingest Plan

_Drafted: 2026-02-20 - Owner: Core Protocol WG / Storage Team / DA WG_

The DA-2 workstream extends Torii with a blob ingest API that emits Norito
metadata and seeds SoraFS replication. This document captures the proposed
schema, API surface, and validation flow so implementation can proceed without
blocking on outstanding simulations (DA-1 follow-ups). All payload formats MUST
use Norito codecs; no serde/JSON fallbacks are permitted.

## Goals

- Accept large blobs (Taikai segments, lane sidecars, governance artefacts)
  deterministically over Torii.
- Produce canonical Norito manifests describing the blob, codec parameters,
  erasure profile, and retention policy.
- Persist chunk metadata in SoraFS hot storage and enqueue replication jobs.
- Publish pin intents + policy tags to the SoraFS registry and governance
  observers.
- Expose admission receipts so clients regain deterministic proof of publication.

## API Surface (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Payload is a Norito-encoded `DaIngestRequest`. Responses use
`application/norito+v1` and return `DaIngestReceipt`.

| Response | Meaning |
| --- | --- |
| 202 Accepted | Blob queued for chunking/replication; receipt returned. |
| 400 Bad Request | Schema/size violation (see validation checks). |
| 401 Unauthorized | Missing/invalid API token. |
| 409 Conflict | Duplicate `client_blob_id` with mismatched metadata. |
| 413 Payload Too Large | Exceeds configured blob length limit. |
| 429 Too Many Requests | Rate limit hit. |
| 500 Internal Error | Unexpected failure (logged + alert). |

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

Returns a versioned `DaProofPolicyBundle` derived from the current lane catalog.
The bundle advertises `version` (currently `1`), a `policy_hash` (hash of the
ordered policy list), and `policies` entries carrying `lane_id`, `dataspace_id`,
`alias`, and the enforced `proof_scheme` (`merkle_sha256` today; KZG lanes are
rejected by ingest until KZG commitments are available). The block header now
commits to the bundle via `da_proof_policies_hash`, so clients can pin the
active policy set when verifying DA commitments or proofs. Fetch this endpoint
before building proofs to ensure they match the lane’s policy and the current
bundle hash. Commitment list/prove endpoints carry the same bundle so SDKs
don’t need an extra round-trip to bind a proof to the active policy set.

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

Returns a `DaProofPolicyBundle` carrying the ordered policy list plus a
`policy_hash` so SDKs can pin the version used when a block was produced. The
hash is computed over the Norito-encoded policy array and changes whenever a
lane’s `proof_scheme` is updated, allowing clients to detect drift between
cached proofs and the chain configuration.

## Proposed Norito Schema

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> Implementation note: the canonical Rust representations for these payloads now live under
> `iroha_data_model::da::types`, with request/receipt wrappers in `iroha_data_model::da::ingest`
> and the manifest structure in `iroha_data_model::da::manifest`.

The `compression` field advertises how callers prepared the payload. Torii accepts
`identity`, `gzip`, `deflate`, and `zstd`, transparently decompressing the bytes before
hashing, chunking, and verifying optional manifests.

### Validation Checklist

1. Verify request Norito header matches `DaIngestRequest`.
2. Fail if `total_size` differs from the canonical (decompressed) payload length or exceeds the configured max.
3. Enforce `chunk_size` alignment (power-of-two, <= 2 MiB).
4. Ensure `data_shards + parity_shards` <= global maximum and parity >= 2.
5. `retention_policy.required_replica_count` must respect governance baseline.
6. Signature verification against canonical hash (excluding signature field).
7. Reject duplicate `client_blob_id` unless payload hash + metadata identical.
8. When `norito_manifest` provided, verify schema + hash matches recalculated
   manifest after chunking; otherwise node generates manifest and stores it.
9. Enforce the configured replication policy: Torii rewrites the submitted
   `RetentionPolicy` with `torii.da_ingest.replication_policy` (see
   `replication_policy.md`) and rejects pre-built manifests whose retention
   metadata does not match the enforced profile.

### Chunking & Replication Flow

1. Chunk payload into `chunk_size`, compute BLAKE3 per chunk + Merkle root.
2. Build Norito `DaManifestV1` (new struct) capturing chunk commitments (role/group_id),
   erasure layout (row and column parity counts plus `ipa_commitment`), retention policy,
   and metadata.
3. Queue the canonical manifest bytes under `config.da_ingest.manifest_store_dir`
   (Torii writes `manifest.encoded` files keyed by lane/epoch/sequence/ticket/fingerprint) so SoraFS
   orchestration can ingest them and link the storage ticket to persisted data.
4. Publish pin intents via `sorafs_car::PinIntent` with governance tag + policy.
5. Emit Norito event `DaIngestPublished` to notify observers (light clients,
   governance, analytics).
6. Return `DaIngestReceipt` (signed by Torii DA service key) and add the
   `Sora-PDP-Commitment` response header containing the base64 Norito encoding
   of the derived commitment so SDKs can stash the sampling seed immediately.
   The receipt now embeds `rent_quote` (a `DaRentQuote`) and `stripe_layout`
   so submitters can surface the XOR obligations, reserve share, PDP/PoTR bonus expectations,
   and the 2D erasure matrix dimensions alongside the storage-ticket metadata before committing funds.
7. Optional registry metadata:
   - `da.registry.alias` — public, unencrypted UTF-8 alias string to seed the pin registry entry.
   - `da.registry.owner` — public, unencrypted `AccountId` string to record registry ownership.
   Torii copies these into the generated `DaPinIntent` so downstream pin processing can bind aliases
   and owners without re-parsing the raw metadata map; malformed or empty values are rejected during
   ingest validation.

## Storage / Registry Updates

- Extend `sorafs_manifest` with `DaManifestV1`, enabling deterministic parsing.
- Add new registry stream `da.pin_intent` with versioned payload referencing
  manifest hash + ticket id.
- Update observability pipelines to track ingest latency, chunking throughput,
  replication backlog, and failure counts.
- Torii `/status` responses now include a `taikai_ingest` array that surfaces the latest
  encoder-to-ingest latency, live-edge drift, and error counters per (cluster, stream), enabling DA-9
  dashboards to ingest health snapshots directly from nodes without scraping Prometheus.

## Testing Strategy

- Unit tests for schema validation, signature checks, duplicate detection.
- Golden tests verifying Norito encoding of `DaIngestRequest`, manifest, and receipt.
- Integration harness spinning up mock SoraFS + registry, asserting chunk + pin flows.
- Property tests covering random erasure profiles and retention combinations.
- Fuzzing of Norito payloads to guard against malformed metadata.
- Golden fixtures for every blob class live under
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` with a companion chunk
  listing in `fixtures/da/ingest/sample_chunk_records.txt`. The ignored test
  `regenerate_da_ingest_fixtures` refreshes the fixtures, while
  `manifest_fixtures_cover_all_blob_classes` fails as soon as a new `BlobClass` variant is added
  without updating the Norito/JSON bundle. This keeps Torii, SDKs, and docs honest whenever DA-2
  accepts a new blob surface.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI & SDK Tooling (DA-8)

- `iroha app da submit` (new CLI entrypoint) now wraps the shared ingest builder/publisher so operators
  can ingest arbitrary blobs outside of the Taikai bundle flow. The command lives in
  `crates/iroha_cli/src/commands/da.rs:1` and consumes a payload, erasure/retention profile, and
  optional metadata/manifest files before signing the canonical `DaIngestRequest` with the CLI
  config key. Successful runs persist `da_request.{norito,json}` and `da_receipt.{norito,json}` under
  `artifacts/da/submission_<timestamp>/` (override via `--artifact-dir`) so release artefacts can
  record the exact Norito bytes used during ingestion.
- The command defaults to `client_blob_id = blake3(payload)` but accepts overrides via
  `--client-blob-id`, honours metadata JSON maps (`--metadata-json`) and pre-generated manifests
  (`--manifest`), and supports `--no-submit` for offline preparation plus `--endpoint` for custom
  Torii hosts. Receipt JSON is printed to stdout in addition to being written to disk, closing the
  DA-8 “submit_blob” tooling requirement and unblocking SDK parity work.
- `iroha app da get` adds a DA-focused alias for the multi-source orchestrator that already powers
  `iroha app sorafs fetch`. Operators can point it at manifest + chunk-plan artefacts (`--manifest`,
  `--plan`, `--manifest-id`) **or** simply pass a Torii storage ticket via `--storage-ticket`. When the
  ticket path is used the CLI pulls the manifest from `/v1/da/manifests/<ticket>`, persists the bundle
  under `artifacts/da/fetch_<timestamp>/` (override with `--manifest-cache-dir`), derives the **manifest
  hash** for `--manifest-id`, and then runs the orchestrator with the supplied `--gateway-provider`
  list. Payload verification still relies on the embedded CAR/`blob_hash` digest while the gateway id is
  now the manifest hash so clients and validators share a single blob identifier. All advanced knobs from
  the SoraFS fetcher surface intact (manifest envelopes, client labels, guard caches, anonymity transport
  overrides, scoreboard export, and `--output` paths), and the manifest endpoint can be overridden via
  `--manifest-endpoint` for custom Torii hosts, so end-to-end availability checks live entirely under the
  `da` namespace without duplicating orchestrator logic.
- `iroha app da get-blob` pulls canonical manifests straight from Torii via `GET /v1/da/manifests/{storage_ticket}`.
  The command now labels artefacts with the manifest hash (blob id), writing
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json`, and `chunk_plan_{manifest_hash}.json`
  under `artifacts/da/fetch_<timestamp>/` (or a user-supplied `--output-dir`) while echoing the exact
  `iroha app da get` invocation (including `--manifest-id`) required for the follow-up orchestrator fetch.
  This keeps operators out of the manifest spool directories and guarantees the fetcher always uses the
  signed artefacts emitted by Torii. The JavaScript Torii client mirrors this flow via
  `ToriiClient.getDaManifest(storageTicketHex)` while the Swift SDK now exposes
  `ToriiClient.getDaManifestBundle(...)`. Both return the decoded Norito bytes, manifest JSON, manifest hash,
  and chunk plan so SDK callers can hydrate orchestrator sessions without shelling out to the CLI, and Swift
  clients can additionally call `fetchDaPayloadViaGateway(...)` to pipe those bundles through the native
  SoraFS orchestrator wrapper.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v1/da/manifests` responses now surface `manifest_hash`, and both CLI + SDK helpers (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway`, and the Swift/JS gateway wrappers) treat this digest as the
  canonical manifest identifier while continuing to verify payloads against the embedded CAR/blob hash.
- `iroha app da rent-quote` computes deterministic rent and incentive breakdowns for a supplied storage size
  and retention window. The helper consumes either the active `DaRentPolicyV1` (JSON or Norito bytes) or
  the built-in default, validates the policy, and prints a JSON summary (`gib`, `months`, policy metadata,
  and the `DaRentQuote` fields) so auditors can cite exact XOR charges inside governance minutes without
  writing ad hoc scripts. The command now also emits a one-line `rent_quote ...` summary before the JSON
  payload to make console logs and runbooks easier to scan when quotes are generated during incidents.
  Pass `--quote-out artifacts/da/rent_quotes/<stamp>.json` (or any other path)
  to persist the pretty-printed summary and use `--policy-label "governance ticket #..."` when the
  artefact needs to cite a specific vote/config bundle; the CLI trims custom labels and rejects blank
  strings to keep `policy_source` values meaningful in evidence bundles. See
  `crates/iroha_cli/src/commands/da.rs` for the subcommand and `docs/source/da/rent_policy.md`
  for the policy schema.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- Pin registry parity now extends to SDKs: `ToriiClient.registerSorafsPinManifest(...)` in the
  JavaScript SDK builds the exact payload used by `iroha app sorafs pin register`, enforcing canonical
  chunker metadata, pin policies, alias proofs, and successor digests before POSTing to
  `/v1/sorafs/pin/register`. This keeps CI bots and automation from shelling out to the CLI when
  recording manifest registrations, and the helper ships with TypeScript/README coverage so DA-8’s
  “submit/get/prove” tooling parity is fully satisfied on JS alongside Rust/Swift.【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` chains all of the above: it takes a storage ticket, downloads the
  canonical manifest bundle, runs the multi-source orchestrator (`iroha app sorafs fetch`) against the
  supplied `--gateway-provider` list, persists the downloaded payload + scoreboard under
  `artifacts/da/prove_availability_<timestamp>/`, and immediately invokes the existing PoR helper
  (`iroha app da prove`) using the fetched bytes. Operators can tweak the orchestrator knobs
  (`--max-peers`, `--scoreboard-out`, manifest endpoint overrides) and the proof sampler
  (`--sample-count`, `--leaf-index`, `--sample-seed`) while a single command produces the artefacts
  expected by DA-5/DA-9 audits: payload copy, scoreboard evidence, and JSON proof summaries.
- `da_reconstruct` (new in DA-6) reads a canonical manifest plus the chunk directory emitted by the chunk
  store (`chunk_{index:05}.bin` layout) and deterministically reassembles the payload while verifying
  every Blake3 commitment. The CLI lives under `crates/sorafs_car/src/bin/da_reconstruct.rs` and ships as
  part of the SoraFS tooling bundle. Typical flow:
  1. `iroha app da get-blob --storage-ticket <ticket>` to download `manifest_<manifest_hash>.norito` and the chunk plan.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (or `iroha app da prove-availability`, which writes the fetch artefacts under
     `artifacts/da/prove_availability_<ts>/` and persists per-chunk files inside the `chunks/` directory).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  A regression fixture lives under `fixtures/da/reconstruct/rs_parity_v1/` and captures the full manifest
  and chunk matrix (data + parity) used by `tests::reconstructs_fixture_with_parity_chunks`. Regenerate it with

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  The fixture emits:

  - `manifest.{norito.hex,json}` — canonical `DaManifestV1` encodings.
  - `chunk_matrix.json` — ordered index/offset/length/digest/parity rows for doc/testing references.
  - `chunks/` — `chunk_{index:05}.bin` payload slices for both data and parity shards.
  - `payload.bin` — deterministic payload used by the parity-aware harness test.
  - `commitment_bundle.{json,norito.hex}` — sample `DaCommitmentBundle` with a deterministic KZG commitment for docs/tests.

  The harness refuses missing or truncated chunks, checks the final payload Blake3 hash against `blob_hash`,
  and emits a summary JSON blob (payload bytes, chunk count, storage ticket) so CI can assert reconstruction
  evidence. This closes the DA-6 requirement for a deterministic reconstruction tool that operators and QA
  jobs can invoke without wiring bespoke scripts.

## TODO Resolution Summary

All previously blocked ingest TODOs have been implemented and verified:

- **Compression hints** — Torii accepts caller-provided labels (`identity`, `gzip`, `deflate`,
  `zstd`) and normalises payloads before validation so the canonical manifest hash matches the
  decompressed bytes.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Governance-only metadata encryption** — Torii now encrypts governance metadata with the
  configured ChaCha20-Poly1305 key, rejects mismatched labels, and surfaces two explicit
  configuration knobs (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) to keep rotation deterministic.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Large payload streaming** — multi-part ingestion is live. Clients stream deterministic
  `DaIngestChunk` envelopes keyed by `client_blob_id`, Torii validates each slice, stages them
  under `manifest_store_dir`, and atomically rebuilds the manifest once the `is_last` flag lands,
  eliminating the RAM spikes seen with single-call uploads.【crates/iroha_torii/src/da/ingest.rs:392】
- **Manifest versioning** — `DaManifestV1` carries an explicit `version` field and Torii refuses
  unknown versions, guaranteeing deterministic upgrades when new manifest layouts ship.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR hooks** — PDP commitments derive directly from the chunk store and are persisted
  beside manifests so DA-5 schedulers can launch sampling challenges from canonical data; the
  `Sora-PDP-Commitment` header now ships with both `/v1/da/ingest` and `/v1/da/manifests/{ticket}`
  responses so SDKs immediately learn the signed commitment that future probes will reference.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】
- **Shard cursor journal** — lane metadata may specify `da_shard_id` (defaulting to `lane_id`), and
  Sumeragi now persists the highest `(epoch, sequence)` per `(shard_id, lane_id)` into
  `da-shard-cursors.norito` alongside the DA spool so restarts drop resharded/unknown lanes and keep
  replay deterministic. The in-memory shard cursor index now fails fast on commitments for
  unmapped lanes instead of defaulting to the lane id, making cursor advancement and replay errors
  explicit, and block validation rejects shard-cursor regressions with a dedicated
  `DaShardCursorViolation` reason + telemetry labels for operators. Startup/catch-up now halts DA
  index hydration if Kura contains an unknown lane or regressing cursor and records the offending
  block height so operators can remediate before serving DA state.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Shard cursor lag telemetry** — the `da_shard_cursor_lag_blocks{lane,shard}` gauge reports how
  far a shard trails the height being validated. Missing/stale/unknown lanes set the lag to the
  required height (or delta), and successful advances reset it to zero so steady-state stays flat.
  Operators should alarm on non-zero lags, inspect the DA spool/journal for the offending lane,
  and verify the lane catalog for accidental resharding before replaying the block to clear the
  gap.
- **Confidential compute lanes** — lanes marked with
  `metadata.confidential_compute=true` and a `confidential_key_version` are treated as
  SMPC/encrypted DA paths: Sumeragi enforces non-zero payload/manifest digests and storage tickets,
  rejects full-replica storage profiles, and indexes the SoraFS ticket + policy version without
  exposing payload bytes. Receipts hydrate from Kura during replay so validators recover the same
  confidentiality metadata after restarts.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/state.rs】

## Implementation Notes

- Torii’s `/v1/da/ingest` endpoint now normalises payload compression, enforces the replay cache,
  deterministically chunks the canonical bytes, rebuilds `DaManifestV1`, and drops the encoded payload
  into `config.da_ingest.manifest_store_dir` for SoraFS orchestration before issuing the receipt; the
  handler also attaches a `Sora-PDP-Commitment` header so clients can capture the encoded commitment
  immediately.【crates/iroha_torii/src/da/ingest.rs:220】
- After persisting the canonical `DaCommitmentRecord`, Torii now emits a
  `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` file beside the manifest spool.
  Each entry bundles the record with the raw Norito `PdpCommitment` bytes so DA-3 bundle builders and
  DA-5 schedulers ingest identical inputs without re-reading manifests or chunk stores.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK helpers expose the PDP header bytes without forcing every client to reimplement Norito parsing:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` cover Rust, the Python `ToriiClient`
  now exports `decode_pdp_commitment_header`, and `IrohaSwift` ships matching helpers so mobile
  clients can stash the encoded sampling schedule immediately.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii also exposes `GET /v1/da/manifests/{storage_ticket}` so SDKs and operators can fetch manifests
  and chunk plans without touching the node’s spool directory. The response returns the Norito bytes
  (base64), rendered manifest JSON, a `chunk_plan` JSON blob ready for `sorafs fetch`, plus the relevant
  hex digests (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) so downstream tooling can
  feed the orchestrator without recomputing digests, and emits the same `Sora-PDP-Commitment` header to
  mirror ingest responses. Passing `block_hash=<hex>` as a query parameter returns a deterministic
  `sampling_plan` rooted in `block_hash || client_blob_id` (shared across validators) containing the
  `assignment_hash`, the requested `sample_window`, and sampled `(index, role, group)` tuples spanning
  the entire 2D stripe layout so PoR samplers and validators can replay the same indices. The sampler
  mixes `client_blob_id`, `chunk_root`, and `ipa_commitment` into the assignment hash; `iroha app da get
  --block-hash <hex>` now writes `sampling_plan_<ticket>.json` next to the manifest + chunk plan with
  the hash preserved, and the JS/Swift Torii clients expose the same `assignment_hash_hex` so validators
  and provers share a single deterministic probe set. When Torii returns a sampling plan, `iroha app da
  prove-availability` now reuses that deterministic probe set (seed derived from `sample_seed`) instead
  of ad-hoc sampling so PoR witnesses line up with validator assignments even if the operator omits a
  `--block-hash` override.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### Large Payload Streaming Flow

Clients that need to ingest assets larger than the configured single-request limit initiate a
streaming session by calling `POST /v1/da/ingest/chunk/start`. Torii responds with a
`ChunkSessionId` (BLAKE3-derived from the requested blob metadata) and the negotiated chunk size.
Each subsequent `DaIngestChunk` request carries:

- `client_blob_id` — identical to the final `DaIngestRequest`.
- `chunk_session_id` — ties slices to the running session.
- `chunk_index` and `offset` — enforce deterministic ordering.
- `payload` — up to the negotiated chunk size.
- `payload_hash` — BLAKE3 hash of the slice so Torii can validate without buffering the entire blob.
- `is_last` — indicates the terminal slice.

Torii persists validated slices under `config.da_ingest.manifest_store_dir/chunks/<session>/` and
records progress inside the replay cache to honour idempotency. When the final slice lands, Torii
reassembles the payload on disk (streaming through the chunk directory to avoid memory spikes),
computes the canonical manifest/receipt exactly as with single-shot uploads, and finally responds to
`POST /v1/da/ingest` by consuming the staged artifact. Failed sessions can be aborted explicitly or
are garbage-collected after `config.da_ingest.replay_cache_ttl`. This design keeps the network format
Norito-friendly, avoids client-specific resumable protocols, and reuses the existing manifest pipeline
unchanged.

**Implementation status.** The canonical Norito types now live in
`crates/iroha_data_model/src/da/`:

- `ingest.rs` defines `DaIngestRequest`/`DaIngestReceipt`, together with the
  `ExtraMetadata` container used by Torii.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` hosts `DaManifestV1` and `ChunkCommitment`, which Torii emits after
  chunking completes.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` provides shared aliases (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile`, etc.) and encodes the default policy values documented below.【crates/iroha_data_model/src/da/types.rs:240】
- Manifest spool files land in `config.da_ingest.manifest_store_dir`, ready for the SoraFS orchestration
  watcher to pull into storage admission.【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi enforces manifest availability when sealing or validating DA bundles:
  blocks fail validation if the spool is missing the manifest or the hash differs
  from the commitment.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

Roundtrip coverage for the request, manifest, and receipt payloads is tracked in
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, ensuring the Norito codec
remains stable across updates.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Retention defaults.** Governance ratified the initial retention policy during
SF-6; the defaults enforced by `RetentionPolicy::default()` are:

- hot tier: 7 days (`604_800` seconds)
- cold tier: 90 days (`7_776_000` seconds)
- required replicas: `3`
- storage class: `StorageClass::Hot`
- governance tag: `"da.default"`

Downstream operators must override these values explicitly when a lane adopts
stricter requirements.

## Rust client proof artefacts

SDKs that embed the Rust client no longer need to shell out to the CLI to
produce the canonical PoR JSON bundle. The `Client` exposes two helpers:

- `build_da_proof_artifact` returns the exact structure generated by
  `iroha app da prove --json-out`, including the manifest/payload annotations supplied
  via [`DaProofArtifactMetadata`].【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` wraps the builder and persists the artefact to disk
  (pretty JSON + trailing newline by default) so automation can attach the file
  to releases or governance evidence bundles.【crates/iroha/src/client.rs:3653】

### Example

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

The JSON payload that leaves the helper matches the CLI down to the field names
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest`, etc.), so existing
automation can diff/parquet/upload the file without format-specific branches.

## Proof verification benchmark

Use the DA proof benchmark harness to validate verifier budgets on representative payloads before
tightening block-level caps:

- `cargo xtask da-proof-bench` rebuilds the chunk store from the manifest/payload pair, samples PoR
  leaves, and times verification against the configured budget. Taikai metadata is auto-filled, and the
  harness falls back to a synthetic manifest if the fixture pair is inconsistent. When `--payload-bytes`
  is set without an explicit `--payload`, the generated blob is written to
  `artifacts/da/proof_bench/payload.bin` so fixtures stay untouched.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- Reports default to `artifacts/da/proof_bench/benchmark.{json,md}` and include proofs/run, total and
  per-proof timings, budget pass rate, and a recommended budget (110% of the slowest iteration) to
  line up with `zk.halo2.verifier_budget_ms`.【artifacts/da/proof_bench/benchmark.md:1】
- Latest run (synthetic 1 MiB payload, 64 KiB chunks, 32 proofs/run, 10 iterations, 250 ms budget)
  recommended a 3 ms verifier budget with 100% of iterations inside the cap.【artifacts/da/proof_bench/benchmark.md:1】
- Example (generates a deterministic payload and writes both reports):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

Block assembly enforces the same budgets: `sumeragi.da_max_commitments_per_block` and
`sumeragi.da_max_proof_openings_per_block` gate the DA bundle before it is embedded in a block, and
each commitment must carry a non-zero `proof_digest`. The guard treats the bundle length as the
proof-opening count until explicit proof summaries are threaded through consensus, keeping the
≤128-opening target enforceable at the block boundary.【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR failure handling and slashing

Storage workers now surface PoR failure streaks and bonded slash recommendations alongside each
verdict. Consecutive failures above the configured strike threshold emit a recommendation that
includes the provider/manifest pair, the streak length that triggered the slash, and the proposed
penalty computed from the provider bond and `penalty_bond_bps`; cooldown windows (seconds) keep
duplicate slashes from firing on the same incident.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs:343】

- Configure thresholds/cooldown via the storage worker builder (defaults mirror the governance
  penalty policy).
- Slash recommendations are recorded in the verdict summary JSON so governance/auditors can attach
  them to evidence bundles.
- Stripe layout + per-chunk roles are now threaded through Torii’s storage pin endpoint
  (`stripe_layout` + `chunk_roles` fields) and persisted into the storage worker so
  auditors/repair tooling can plan row/column repairs without re-deriving layout from upstream

### Placement + repair harness

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` now
computes a placement hash over `(index, role, stripe/column, offsets)` and performs row-first then
column RS(16) repair before reconstructing the payload:

- Placement defaults to `total_stripes`/`shards_per_stripe` when present and falls back to chunk
- Missing/corrupted chunks are rebuilt with row parity first; remaining gaps are repaired with
  stripe (column) parity. Repaired chunks are written back to the chunk directory, and the JSON
  summary captures the placement hash plus row/column repair counters.
- If row+column parity cannot satisfy the missing set, the harness fails fast with the unrecoverable
  indices so auditors can flag irreparable manifests.
