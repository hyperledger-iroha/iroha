title: Data Availability Ingest Plan
sidebar_label: Ingest Plan
description: Schema, API surface, and validation plan for Torii blob ingestion.
---

:::note Canonical Source
:::

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
POST /v2/da/ingest
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
   `replication-policy.md`) and rejects pre-built manifests whose retention
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
6. Return `DaIngestReceipt` to caller (signed by the Torii DA service key) and emit the
   `Sora-PDP-Commitment` header so SDKs can capture the encoded commitment immediately. The receipt
   now includes `rent_quote` (a Norito `DaRentQuote`) and `stripe_layout`, letting submitters display
   the base rent, reserve share, PDP/PoTR bonus expectations, and the 2D erasure layout alongside
   the storage ticket before committing funds.

## Storage / Registry Updates

- Extend `sorafs_manifest` with `DaManifestV1`, enabling deterministic parsing.
- Add new registry stream `da.pin_intent` with versioned payload referencing
  manifest hash + ticket id.
- Update observability pipelines to track ingest latency, chunking throughput,
  replication backlog, and failure counts.

## Testing Strategy

- Unit tests for schema validation, signature checks, duplicate detection.
- Golden tests verifying Norito encoding of `DaIngestRequest`, manifest, and receipt.
- Integration harness spinning up mock SoraFS + registry, asserting chunk + pin flows.
- Property tests covering random erasure profiles and retention combinations.
- Fuzzing of Norito payloads to guard against malformed metadata.

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
  `--plan`, `--manifest-id`) **or** pass a Torii storage ticket via `--storage-ticket`. When the ticket
  path is used the CLI pulls the manifest from `/v2/da/manifests/<ticket>`, persists the bundle under
  `artifacts/da/fetch_<timestamp>/` (override with `--manifest-cache-dir`), derives the blob hash for
  `--manifest-id`, and then runs the orchestrator with the supplied `--gateway-provider` list. All
  advanced knobs from the SoraFS fetcher surface intact (manifest envelopes, client labels, guard caches,
  anonymity transport overrides, scoreboard export, and `--output` paths), and the manifest endpoint can
  be overridden via `--manifest-endpoint` for custom Torii hosts, so end-to-end availability checks live
  entirely under the `da` namespace without duplicating orchestrator logic.
- `iroha app da get-blob` pulls canonical manifests straight from Torii via `GET /v2/da/manifests/{storage_ticket}`.
  The command writes `manifest_{ticket}.norito`, `manifest_{ticket}.json`, and `chunk_plan_{ticket}.json`
  under `artifacts/da/fetch_<timestamp>/` (or a user-supplied `--output-dir`) while echoing the exact
  `iroha app da get` invocation (including `--manifest-id`) required for the follow-up orchestrator fetch.
  This keeps operators out of the manifest spool directories and guarantees the fetcher always uses the
  signed artefacts emitted by Torii. The JavaScript Torii client mirrors this flow via
  `ToriiClient.getDaManifest(storageTicketHex)`, returning the decoded Norito bytes, manifest JSON,
  and chunk plan so SDK callers can hydrate orchestrator sessions without shelling out to the CLI.
  The Swift SDK now exposes the same surfaces (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), piping bundles into the native SoraFS orchestrator wrapper so
  iOS clients can download manifests, execute multi-source fetches, and capture proofs without
  invoking the CLI.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` computes deterministic rent and incentive breakdowns for a supplied storage size
  and retention window. The helper consumes either the active `DaRentPolicyV1` (JSON or Norito bytes) or
  the built-in default, validates the policy, and prints a JSON summary (`gib`, `months`, policy metadata,
  and the `DaRentQuote` fields) so auditors can cite exact XOR charges inside governance minutes without
  writing ad hoc scripts. The command also emits a one-line `rent_quote ...` summary ahead of the JSON
  payload to keep console logs readable during incident drills. Pair `--quote-out artifacts/da/rent_quotes/<stamp>.json` with
  `--policy-label "governance ticket #..."` to persist prettified artefacts that cite the exact policy vote
  or config bundle; the CLI trims the custom label and refuses blank strings so `policy_source` values
  remain actionable across treasury dashboards. See `crates/iroha_cli/src/commands/da.rs` for the subcommand
  and `docs/source/da/rent_policy.md` for the policy schema.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` chains all of the above: it takes a storage ticket, downloads the
  canonical manifest bundle, runs the multi-source orchestrator (`iroha app sorafs fetch`) against the
  supplied `--gateway-provider` list, persists the downloaded payload + scoreboard under
  `artifacts/da/prove_availability_<timestamp>/`, and immediately invokes the existing PoR helper
  (`iroha app da prove`) using the fetched bytes. Operators can tweak the orchestrator knobs
  (`--max-peers`, `--scoreboard-out`, manifest endpoint overrides) and the proof sampler
  (`--sample-count`, `--leaf-index`, `--sample-seed`) while a single command produces the artefacts
  expected by DA-5/DA-9 audits: payload copy, scoreboard evidence, and JSON proof summaries.

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
  beside manifests so DA-5 schedulers can launch sampling challenges from canonical data, and
  `/v2/da/ingest` plus `/v2/da/manifests/{ticket}` now include a `Sora-PDP-Commitment` header
  carrying the base64 Norito payload so SDKs cache the exact commitment DA-5 probes target.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## Implementation Notes

- Torii’s `/v2/da/ingest` endpoint now normalises payload compression, enforces the replay cache,
  deterministically chunks the canonical bytes, rebuilds `DaManifestV1`, drops the encoded payload
  into `config.da_ingest.manifest_store_dir` for SoraFS orchestration, and adds the `Sora-PDP-Commitment`
  header so operators capture the commitment that PDP schedulers will reference.【crates/iroha_torii/src/da/ingest.rs:220】
- Every accepted blob now produces a `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito`
  entry under `manifest_store_dir` bundling the canonical `DaCommitmentRecord` together with the raw
  `PdpCommitmentV1` bytes so DA-3 bundle builders and DA-5 schedulers hydrate identical inputs without
  re-reading manifests or chunk stores.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK helper APIs expose the PDP header payload without forcing callers to reimplement Norito decoding:
  the Rust crate exports `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`, the Python
  `ToriiClient` now includes `decode_pdp_commitment_header`, and `IrohaSwift` ships
  `decodePdpCommitmentHeader` overloads for raw header maps or `HTTPURLResponse` instances.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii also exposes `GET /v2/da/manifests/{storage_ticket}` so SDKs and operators can fetch manifests
  and chunk plans without touching the node’s spool directory. The response returns the Norito bytes
  (base64), rendered manifest JSON, a `chunk_plan` JSON blob ready for `sorafs fetch`, the relevant
  hex digests (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`), and mirrors the
  `Sora-PDP-Commitment` header from ingest responses for parity. Supplying `block_hash=<hex>` in the
  query string returns a deterministic `sampling_plan` (assignment hash, `sample_window`, and sampled
  `(index, role, group)` tuples spanning the full 2D layout) so validators and PoR tools draw the same
  indices.

### Large Payload Streaming Flow

Clients that need to ingest assets larger than the configured single-request limit initiate a
streaming session by calling `POST /v2/da/ingest/chunk/start`. Torii responds with a
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
`POST /v2/da/ingest` by consuming the staged artifact. Failed sessions can be aborted explicitly or
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
