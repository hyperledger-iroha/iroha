## SoraFS Node Storage Implementation Notes

This note refines how an Iroha (Torii) node can opt-in to the SoraFS data
availability layer and dedicate a slice of local disk for storing and serving
chunks. It complements the `sorafs_node_client_protocol.md` discovery spec and
the SF-1b fixture work by outlining the storage-side architecture, resource
controls, and configuration plumbing implemented across the node and gateway
code paths. Practical operator drills live in
`sorafs/runbooks/sorafs_node_ops.md`.

### Goals

- Allow any validator or auxiliary Iroha process to expose spare disk as a
  SoraFS provider without affecting the core ledger responsibilities.
- Keep the storage module deterministic and Norito-driven: manifests,
  chunk plans, Proof-of-Retrievability (PoR) roots, and provider adverts are the
  source of truth.
- Enforce operator-defined quotas so a node cannot exhaust its own resources by
  accepting too many pin or fetch requests.
- Surface health/telemetry (PoR sampling, chunk fetch latency, disk pressure)
  back to governance and clients.

### High-level Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Key modules:

- **Gateway**: exposes Norito HTTP endpoints for pin proposals, chunk fetch
  requests, PoR sampling, and telemetry. It validates Norito payloads and
  marshals requests into the chunk store. Reuses the existing Torii HTTP stack
  to avoid a new daemon.
- **Pin Registry**: the manifest pin state tracked in `iroha_data_model::sorafs`
  and `iroha_core`. When a manifest is accepted the registry records the
  manifest digest, chunk plan digest, PoR root, and provider capability flags.
- **Chunk Storage**: disk-backed `ChunkStore` implementation that ingests
  signed manifests, materialises chunk plans using `ChunkProfile::DEFAULT`, and
  persists chunks under a deterministic layout. Each chunk is associated with a
  content fingerprint and PoR metadata. The first-release backend verifies the
  complete chunk from a no-follow file descriptor before serving bytes or using
  them in a PoR proof, so same-length bit flips and symlink replacement fail
  closed instead of being served under an immutable CID.
- **Quota/Scheduler**: enforces operator-configured limits (maximum disk bytes,
  maximum outstanding pins, maximum parallel fetches, chunk TTL) and coordinates
  IO so the node's ledger duties are not starved. The scheduler is also
  responsible for serving PoR proofs and sampling requests with bounded CPU.

### Configuration

Add a new section to `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
pdp_sample_window = 64
pdp_tree_memory_limit_bytes = "512 MiB"
reputation_trust_policy_path = "/etc/iroha/sorafs-reputation-trust-policy.to"
moderation_screening_enabled = false
moderation_screening_authority_bundle_path = "/etc/iroha/sorafs-screening-authority.to"
moderation_screening_authority_bundle_digest_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
alias = "tenant.alpha"            # optional human friendly tag

[sorafs.storage.runtime]
event_history_limit = 4_096
state_entry_limit = 65_536
checkpoint_max_bytes = "64 MiB"
proof_outcome_forwarder_interval_ms = 1_000
proof_outcome_max_attempts = 8

adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: participation toggle. When false the gateway returns a 503 for
  storage endpoints and the node does not advertise in discovery.
- `data_dir`: root directory for chunk data, PoR trees, and fetch telemetry.
  Defaults to `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: hard limit for pinned chunk data. A background task
  rejects new pins when the limit is reached.
- `max_parallel_fetches`: concurrency cap enforced by the scheduler to balance
  bandwidth/disk IO against validator workload.
- `max_pins`: maximum number of manifest pins the node accepts before applying
  eviction/back pressure.
- `por_sample_interval_secs`: cadence for automatic PoR sampling jobs. Each job
  samples `N` leaves (configurable per manifest) and emits telemetry events.
  Governance can scale `N` deterministically by setting the capacity metadata
  key `profile.sample_multiplier` (integer `1-4`). The value may be a single
  number/string or an object with per-profile overrides, e.g.
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
  The unauthenticated local `/v1/sorafs/storage/por-sample` probe is retired.
  Manual production probes use authenticated `POST /v1/sorafs/proof/stream`;
  PoR `sample_count` is limited to `1..=500`, and Torii requires an approved
  finalized pin record before sampling and verifying against its committed root.
- `pdp_sample_window`: maximum number of distinct PDP segments admitted in one
  governed challenge. Configuration parsing rejects zero and values above the
  protocol ceiling of 500 before the storage worker starts.
- `pdp_tree_memory_limit_bytes`: checked aggregate budget for the exact node
  slabs retained by canonical PDP indexes. Each ingest reserves its complete
  retained-tree estimate before reading payload bytes; concurrent attempts see
  those reservations, failed attempts release them, and eviction subtracts the
  evicted tree. CAR ingestion separately validates checked peak-allocation
  geometry for tree construction, PoR metadata, chunk metadata, and sink state.
- `reputation_trust_policy_path`: optional path to a canonical, bounded Norito
  `ReputationSnapshotTrustPolicyV1`. When configured, startup rejects missing,
  symlinked, hard-linked, writable-by-other, oversized, noncanonical, or invalid
  policy files. Signed reputation admission is unavailable when the path is
  absent; there is no unsigned publication fallback.
- `moderation_screening_enabled`: enables authenticated moderation screening
  admission. This requires storage plus both authority-bundle settings below;
  startup fails instead of accepting unsigned or process-local authority. It
  also requires a runtime-injected `ModerationQuarantineKeyWrapper`; neither
  provider credentials nor a software/file key can be supplied through
  `iroha_config`.
- `moderation_screening_authority_bundle_path`: absolute path to the canonical
  Norito `ModerationScreeningAuthorityBundleV1`. The bundle contains the signed
  model manifest, signed policy chain, sorted governance trust anchors, and
  minimum governance quorum. Startup rejects missing, symlinked, hard-linked,
  writable-by-other, replaced, oversized, noncanonical, and invalid inputs.
  In-process authority rotation separately rejects policy rollback and
  same-timestamp equivocation.
- `moderation_screening_authority_bundle_digest_hex`: exact lowercase BLAKE3
  digest of the authority-bundle bytes. It must contain 64 hexadecimal
  characters and cannot be all zeroes. Rotating the bundle therefore requires
  an explicit configuration change and restart.

Quarantine-object plaintext and private notes are carried only inside the
chunked ChaCha20-Poly1305 payload. The optional plaintext `content_type` is
limited to a coarse V1 allowlist (`application/octet-stream`, JSON, PDF, common
image/audio/video formats, and plain text) without parameters, filenames,
identities, or free-form text. The legacy-shaped `notes` request and record
field is reserved and must be absent in V1; requests or checkpoints that
populate it fail closed.

`ToriiRuntimeDeps` and `IrohaRuntimeDeps` expose the PKCS#11/managed-KMS
adapter boundary and validate the injected node's screening enablement,
authority digest, and non-secret active key handle. The standard `irohad`
launcher still supplies no concrete provider adapter, so a screening-enabled
reference deployment intentionally fails startup until
`V1-BLOCK-AI-QUARANTINE-KMS-01` is resolved.
- `runtime.event_history_limit`: per-stream replay ceiling. Repair, reputation,
  orderbook, and moderation histories retain the newest events while keeping a
  separate monotonic high-water sequence. Gap-aware replay reports when a
  cursor predates retained history instead of silently pretending the stream
  is complete.
- `runtime.state_entry_limit`: hard ceiling for each auxiliary PoR,
  reputation, transparency, privacy, processed-cycle, reserve, deal, capacity,
  and orderbook index. This includes deal ticket replay IDs, outstanding
  replication orders, and retained orderbook trades/channels/receipts. New
  authoritative entries are refused at the ceiling; published source events
  are pruned only after their governance publication succeeds.
  Moderation applies the ceiling independently to model manifests, corpora,
  screening/quarantine records, encrypted-object index entries, evidence-viewer
  sessions/access events, and the global ballot, juror, commit, reveal, and
  challenge counts. Idempotent replay and updates to existing moderation keys
  remain available at capacity. Torii maps a new-key refusal to HTTP `429 Too
  Many Requests`; conflicts remain `409` and malformed snapshots remain `400`.
- `runtime.checkpoint_max_bytes`: maximum canonical Norito checkpoint size.
  Oversize, corrupt, symlinked, or non-regular checkpoints fail startup rather
  than resetting durable replay or penalty state.
- `runtime.proof_outcome_forwarder_interval_ms`: finalized-chain reconciliation
  cadence for durable PDP and PoTR outcome delivery.
- `runtime.proof_outcome_max_attempts`: bounded attempts for one exact signed
  outcome transaction before terminal dead-lettering.

The proof-outcome forwarder reconciles against a height-and-block-hash cursor
from one finalized state view. Before it claims or signs a ready delivery, it
also requires the runtime signer's account to hold the exact
provider-scoped `CanRecordSorafsProofOutcome` permission, directly or through
a role, in finalized state. A missing or differently scoped grant defers the
delivery without consuming a retry. The standard `irohad` launcher adapts its
runtime-only common node key at this boundary; reference deployments can inject
a PKCS#11/HSM implementation of `SoraFsProofOutcomeTransactionSigner` without
giving that signer transaction-queue access.
- `adverts`: structure used by the provider advert generator to fill
  `ProviderAdvertV1` fields (stake pointer, QoS hints, topics). If omitted the
  node uses defaults from the governance registry.

Config plumbing:

- `[sorafs.storage]` is defined in `iroha_config` as `SorafsStorage` and is
  loaded from the node config file.
- `iroha_core` and `iroha_torii` thread the storage config into the gateway
  builder and chunk store at startup.
- Dev/test env overrides exist (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), but
  production deployments should rely on the config file.

### CLI Utilities

The `sorafs_node` crate also ships a thin CLI so operators can script
ingestion/export drills against the persistent backend and compare local
outputs with the Torii HTTP surface.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` expects a Norito-encoded manifest `.to` file plus the matching payload
  bytes. It reconstructs the chunk plan from the manifest’s chunking profile,
  enforces digest parity, persists chunk files, and optionally emits a
  strict `sorafs.chunk_fetch_plan.v1` JSON object so downstream tooling can
  verify both the whole-payload BLAKE3 binding and the chunk layout.
- `export` accepts a manifest ID and writes the stored manifest/payload to disk
  (with optional plan JSON) so fixtures remain reproducible across environments.

Both commands print a Norito JSON summary to stdout, making it easy to pipe into
scripts. The CLI is covered by an integration test to ensure manifests and
payloads round-trip cleanly alongside the Torii APIs.【crates/sorafs_node/tests/cli.rs:1】

> HTTP parity
>
> The Torii gateway now exposes read-only helpers backed by the same
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id}` — returns the stored
>   Norito manifest (base64) alongside digest/metadata. Supplying `?limit=N`
>   bounds the returned `files` metadata array (max 500) while preserving
>   `file_count`/`returned_file_count`/`truncated_files`; omitting `limit`
>   returns the complete file list for remote cache compatibility.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id}` — returns a bounded diagnostic
>   projection of deterministic chunk metadata for downstream inspection; it is
>   not a standalone fetch-plan input. The `files`,
>   `chunk_digests_blake3`, and `chunks` arrays are bounded by `limit` (default
>   50, max 500), with full count/returned count/truncation metadata for
>   inventory probes.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> These endpoints mirror the CLI output so pipelines can switch from local
> scripts to HTTP probes without changing parsers.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Node Lifecycle

1. **Startup**:
   - If storage is enabled the node initialises the chunk store with the
   configured directory and capacity. For every pinned non-empty payload the
   metadata stores a validated `PdpCommitmentV1` and a bounded PoR commitment
   summary (root plus exact global/per-chunk geometry), while the index stores
   domain-separated digests of both commitments. PoR leaves, segments, and
   Merkle node slabs are never persisted; metadata remains proportional to the
   content-chunk inventory rather than the 4 KiB proof-leaf inventory.
   Startup reads every chunk through the bounded no-follow verifier, recomputes
   the payload digest, and rebuilds both PoR and PDP trees from those bytes. It
   rejects any mismatch in manifest/profile geometry, persisted PoR state, PDP
   roots/counts, sample window, or seal timestamp before serving data, and sums
   all rebuilt PDP trees against the configured aggregate memory budget.
   - Restore bounded auxiliary runtime state from
     `runtime-state/auxiliary-snapshot.to`. The checkpoint retains PoR penalty
     high-water state, replay sequences, reputation snapshots, reserve
     lifecycle/custody records, deal balances and ticket replay IDs, capacity
     declarations and outstanding reservations, unpublished
     transparency/privacy inputs, and processed publication cycles. Capacity
     restore recomputes per-profile/lane allocations and rebuilds metering
     gauges; deal restore recomputes locked collateral from retained deals.
     Reads are no-follow and size-bounded; writes use create-new staging, file
     fsync, atomic rename, and parent-directory fsync.
   - Moderation ballot mutations commit the ballot record and its sequenced
     event in one checkpoint transaction. Event-lock failure, sequence
     exhaustion, or a pre-rename checkpoint error restores both in-memory
     snapshots and returns an explicit error. Live broadcast, transparency,
     and Governance DAG publication occur only after that checkpoint commits.
   - Rebuild repair work from the finalized native task and typed-event
     projections at one exact height/block-hash anchor. Storage execution is
     permitted only for the reconciled live lease owner, generation, revision,
     provider binding, and expiry. The retired `repair/repair_state.to`,
     `FileRepairStore`, and local `RepairManager` have no loader or migration
     path. GC and reconciliation first prove one complete bounded task
     projection from a single immutable finalized query view. No local
     checkpoint can create, lease, complete, fail, escalate, or appeal a task.
   - Register the SoraFS gateway routes (Norito JSON POST/GET endpoints for pin,
     fetch, PoR sample, telemetry).
   - Spawn the PoR sampling worker and quota monitor.
2. **Discovery / Adverts**:
   - Generate `ProviderAdvertV1` documents using current capacity/health, sign
     them with the council-approved key, and publish via the configured
     discovery channel.
3. **Pin Workflow**:
   - Gateway receives a signed manifest (including chunk plan, PoR root, council
     signatures). Validate the alias list (`sorafs.sf1@1.0.0` required) and
     ensure the chunk plan matches the manifest metadata.
   - Check quotas. If capacity/pin limits would be exceeded respond with a
     policy error (Norito structured).
   - Stream chunk data into the `ChunkStore`, verifying digests as we ingest.
     Update PoR trees and store manifest metadata in the registry.
4. **Fetch Workflow**:
   - Serve chunk range requests from disk. Scheduler enforces
     `max_parallel_fetches` and returns `429` when saturated.
   - Emit structured telemetry (Norito JSON) with latency, bytes served, and
     error counts for downstream monitoring.
5. **PoR Sampling**:
   - Worker selects manifests proportional to weight (e.g., bytes stored) and
     runs deterministic sampling using the chunk store's PoR tree.
   - Persist results for governance audits and include summaries in provider
     adverts / telemetry endpoints.
6. **Eviction / Quota Enforcement**:
   - When capacity is reached the node rejects new pins by default. Optionally,
     operators may configure eviction policies (e.g., TTL-based, LRU) once the
     governance model is agreed; for now the design assumes strict quotas and
     operator-initiated unpin operations.

### Capacity Declaration & Scheduling Integration

- Torii now relays `CapacityDeclarationRecord` updates from `/v1/sorafs/capacity/declare`
  to the embedded `CapacityManager`, so each node builds an in-memory view of its
  committed chunker and lane allocations. The manager exposes read-only snapshots
  for telemetry (`GET /v1/sorafs/capacity/state`) and enforces per-profile or per-lane
  reservations before new orders are accepted.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- The `/v1/sorafs/capacity/schedule` endpoint accepts governance-issued `ReplicationOrderV1`
  payloads. When the order targets the local provider the manager checks for
  duplicate scheduling, verifies chunker/lane capacity, reserves the slice, and
  returns a `ReplicationPlan` describing remaining capacity so orchestration tools
  can proceed with ingestion. Orders for other providers are acknowledged with an
  `ignored` response to ease multi-operator workflows.【crates/iroha_torii/src/routing.rs:4845】
- Completion hooks (e.g., triggered after ingestion succeeds) hit
  `POST /v1/sorafs/capacity/complete` to release reservations via
  `CapacityManager::complete_order`. The response includes a `ReplicationRelease`
  snapshot (remaining totals, chunker/lane residuals) so orchestration tooling can
  queue the next order without polling. The current storage path ingests manifests through
  `NodeHandle::ingest_manifest` and `sorafs-node ingest`, while orchestration can
  use the `sorafs_cli storage prepare`/`storage pin` sequence; call the
  completion hook after those ingestion steps succeed.【crates/iroha_torii/src/routing.rs:34922】【crates/sorafs_node/src/capacity.rs:87】【crates/sorafs_node/src/lib.rs:2168】【crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:6160】
- The embedded `TelemetryAccumulator` can be mutated through
  `NodeHandle::update_telemetry`, letting background workers record PoR/uptime samples
  and eventually derive canonical `CapacityTelemetryV1` payloads without touching the
  scheduler internals.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integrations & Operational Hardening

The persisted storage boundary is fail-closed in v1:

- the backend holds an operating-system exclusive lock for the configured data
  directory for its full lifetime, so a second node process cannot mutate the
  same index or manifest tree concurrently. The lock is opened without
  following symlinks and the opened file identity is rechecked against the
  path before and after locking, closing replacement races;
- same-manifest ingestion has a single in-flight owner and writes into a unique
  attempt directory before publishing the completed directory atomically;
  rejected or failed attempts can clean only their own staging state;
- startup resolves interrupted transactions deterministically: it restores a
  manifest moved to GC while the old index is still authoritative, purges GC
  data after the new index is authoritative, and removes stale staging or
  unindexed ingest directories. Unknown transaction names and symlinked
  transaction directories fail startup instead of being traversed;
- atomic index and metadata replacement pins the opened parent identity before
  publication, renames only beneath that stable identity (`/proc/self/fd` on
  Linux and the volume/file-id namespace on macOS), and syncs both the file and
  parent directory. Linux fails startup if the required procfs descriptor
  namespace is unavailable; macOS opens with all-component no-follow
  semantics. A bounded per-target publication lock keeps rename,
  post-commit identity verification, and directory sync indivisible across
  local writers while allowing temporary-file writes to proceed concurrently.
  If rename succeeds but identity verification or directory sync fails, the
  backend records the uncertain commit, refuses all subsequent reads and
  mutations, and requires restart recovery; it never guesses whether the old
  or new state is authoritative;
- metadata updates are copy-on-write in memory and on disk. A failure before
  rename leaves the live descriptor unchanged, while an uncertain post-rename
  result installs the committed descriptor and immediately fail-stops the
  backend;
- each stored manifest carries a shared I/O lease: fetch, PoR, and manifest
  reads hold a read lease, while metadata mutation and eviction require the
  exclusive lease, preventing deletion or rewrite from racing an active read;
- pin, fetch, and PoR admission is fail-fast at the configured concurrency and
  byte-rate ceilings. Saturated Torii requests receive `429` with
  `Retry-After`; no request thread sleeps or waits on an unbounded scheduler
  queue;
- startup rejects unsupported index versions, duplicate or noncanonical
  manifest IDs, traversal-bearing chunk/file names, inconsistent index,
  manifest, file-layout, chunk, or PoR geometry, and corrupt chunk digests;
- ingest rejects empty inventories, duplicate or non-portable logical paths,
  overflowing or out-of-bounds file/chunk ranges, and layouts whose file bytes
  do not align exactly with the canonical chunk plan;
- index, manifest, and metadata reads open regular files without following the
  leaf symlink and enforce structural byte ceilings before allocation (64 MiB
  for the index and per-manifest metadata, 16 MiB for a manifest envelope);
- byte accounting and chunk reference counts are recomputed with checked
  arithmetic during recovery rather than trusting stale persisted totals, and
  in-memory layout indices must fit their exact on-disk `u32` representation
  instead of being truncated.

- **Governance**: extend `sorafs_pin_registry_tracker.md` with storage telemetry
  (PoR success rate, disk utilisation). Admission policies can require minimum
  capacity or minimum PoR success rate before adverts are accepted.
- **Client SDKs**: expose the new storage config (disk limits, alias) so
  management tooling can bootstrap nodes programmatically.
- **Telemetry**: storage scheduler metrics now export through the existing
  Prometheus/OpenTelemetry stack, including byte usage, queue depth, fetch
  throughput, and PoR sample counters.
- **Security**: run the storage module inside a dedicated async task pool with
  back-pressure and consider sandboxing chunk reads via io_uring or tokio's
  bounded pools to prevent malicious clients from exhausting resources.

This implementation keeps the storage module optional and deterministic while
giving operators the knobs they need to participate in the SoraFS data
availability layer. Outstanding rollout evidence is operational hardening:
hosted deployment captures, governance policy tuning, and SDK management
ergonomics.
