---
lang: fr
direction: ltr
source: docs/source/sorafs_governance_dag_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94e2255abbaa444994c28130d2a7fbd8af6b7865f946a115c9dd711fca06ed3c
source_last_modified: "2026-01-03T18:07:57.749197+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Governance DAG Publishing Pipeline
summary: Final specification for SF-12 detailing append-only DAG ingestion, IPNS publishing, verification, observability, and rollout.
---

# Governance DAG Publishing Pipeline

## Goals & Scope
- Capture every governance artefact (adverts, replication orders, PoR/PoTR events, repairs, verdicts, reports) in an append-only merkle DAG backed by IPLD/CAR files.
- Provide deterministic head pointers via IPNS so operators, SDKs, and auditors can retrieve the latest governance state.
- Ensure block-level signatures and rigorous validation so consumers can trust data without additional context.
- Deliver tooling for exporting snapshots, querying history, and integrating with dashboards.

This document completes **SF-12 — Governance DAG publishing pipeline** and supersedes prior drafts.

## High-Level Architecture
| Component | Responsibility | Implementation Notes |
|-----------|----------------|----------------------|
| Ingest Service (`governance_dag_ingest`) | Subscribes to Torii governance events and repair/PoR notifications; validates payload signatures. | Runs as async Rust service, uses `iroha_torii` client. |
| DAG Builder (`governance_dag_builder`) | Wraps validated payloads into `GovernanceDagNodeV1`, computes parent linkage, writes blocks to local datastore, assembles CAR segments. | Uses libipld, deterministic CID computation (Blake3 multihash). |
| Publisher (`governance_dag_publisher`) | Pins new blocks to IPFS Cluster, updates IPNS head, emits Torii head announcements. | Signs head update with Dilithium3 key. |
| Mirror Datastore | RocksDB-backed local store to enable fast queries and offline verification. | Maintains secondary index by payload kind and manifest CID. |
| Dashboard/API Backend | Serves REST/GraphQL queries aggregating DAG data for dashboards and audit tools. | Optional caching layer built on top of mirror datastore. |
| CLI (`sorafs governance dag`) | Operator tooling to inspect blocks, export snapshots, verify chains, and rebuild head pointers. | Part of reference SDK binary suite. |

## Data Model
- `GovernanceDagNodeV1` Norito struct fields:
  ```norito
  struct GovernanceDagNodeV1 {
      version: u8,                    // 1
      payload_kind: GovernancePayloadKind,
      payload: Vec<u8>,               // raw Norito bytes (adverts, orders, etc.)
      payload_hash: Digest32,         // BLAKE3-256 of payload
      parent_cid: Option<Cid>,        // None for genesis
      timestamp: Timestamp,
      publisher_signature: Signature, // Dilithium3
      publisher_key_id: String,       // key reference
      annotations: Map<String, String>, // optional metadata (e.g., iso_week)
  }
  ```
- `GovernancePayloadKind` enumerates: `Advert`, `ReplicationOrder`, `PorChallenge`, `PorProof`, `DealSettlement`, `PotrProbe`, `RepairEvent`, `GovernanceVerdict`, `WeeklyReport`, `Snapshot`, `PolicyUpdate`, `Custom`.
- Blocks are stored as IPLD DAG-CBOR; CIDs use multicodec `dag-cbor` with multihash `blake3-256`.
- Append-only semantics: each new block references the previous head CID (single linked list) but includes merkle DAG features via payload references (e.g., challenge ↔ proof). Quarterly snapshots record a skip-link to reduce catch-up time.
- Deal settlements are encoded as `DealSettlementV1`, carrying the ledger snapshot
  (`DealLedgerSnapshotV1`) with provider/client identifiers, cumulative accruals,
  remaining bond collateral, and captured-at timestamps so auditors can reconcile
  Torii telemetry with governance history.

## Publishing Workflow
1. **Ingestion**
   - Torii emits governance events via WebSocket/REST. Ingest fetches the full Norito payload.
   - Payload signature verified using registry keys (provider, council, auditor).
   - Deduplication: ingest maintains `seen_payload_hash` LRU (24h) to avoid re-publishing duplicates.
2. **Block Construction**
   - Builder retrieves current head CID from mirror store. 
   - Creates `GovernanceDagNodeV1`, sets `parent_cid`, calculates `payload_hash`.
   - Serialises node to DAG-CBOR, computes block CID.
   - Writes block to RocksDB mirror (key = CID, value = bytes) and appends to CAR segment file (size target 32 MiB).
3. **Validation**
   - Secondary validator process reads block from RocksDB, re-derives CID, checks parent existence, and ensures payload verification marks remain true.
   - If validation fails, block quarantined under `quarantine/<cid>` and pipeline halts pending operator review.
4. **Publishing**
   - CAR segment uploaded to IPFS Cluster (`/pin/add` with replication factor 3).
   - Publisher updates IPNS record `k51qzi5uqu5dl…` (alias `dag.sora.net`) to point to latest head CID (embedded in a small manifest `governance_dag_head.json`).
   - Torii broadcasts `GovernanceDagHeadV1` event (`{head_cid, previous_head_cid, timestamp}`).
5. **Notification**
   - Dashboard backend listens to head events, refreshes caches, updates Grafana widgets.
   - CLI `sorafs governance dag head` fetches IPNS pointer and verifies signature.

## Storage & Retention
- **Primary**: IPFS Cluster (3 nodes) storing CAR files, replication factor 3, GC disabled except via pipeline.
- **Mirror**: RocksDB columns (`blocks`, `payload_index`, `timeline`) on NVMe volumes with 1-year retention.
- **Cold Storage**: Nightly job exports CAR snapshot to S3 `s3://sorafs-governance-dag/YYYY/MM/DD/head.car.zst`; Glacier copy kept for 7 years.
- **Checkpointing**:
  - Every quarter, pipeline emits `Snapshot` block with aggregated manifest listing block CIDs and metadata.
  - Snapshots stored separately for fast bootstrap; CLI supports `sorafs governance dag fetch --snapshot <cid>`.
- **Pruning**:
  - After snapshot success + S3 upload, CAR files older than 6 months removed from IPFS cluster (except snapshots).
  - RocksDB retains index metadata for 12 months; block bytes older than 6 months replaced with stub referencing S3 location.
  - Export CLI handles retrieving missing blocks from S3 automatically.

## Security & Verification
- Payload verification strictly uses Norito definitions (no JSON conversions). Unknown versions -> fail.
- Publisher key: Dilithium3 stored in HSM or sealed Vault. Access only for publisher service.
- Block signature ensures tamper detection even if IPFS content replaced.
- Head manifest includes BLAKE3 digest and signature; IPNS record signed with Ed25519 key managed by publisher.
- Clients must: 
  1. Resolve IPNS -> head manifest.
  2. Verify manifest signature and digest.
  3. Pull blocks via IPFS/S3, validating each `GovernanceDagNodeV1` and payload signature.
- CLI command `sorafs governance dag verify --head <cid> --depth N` automates this process.
- Alerts: `governance_dag_publish_failed`, `governance_dag_validation_error`, `governance_dag_pin_lag`, `governance_dag_ipns_update_failed`.

## Observability
- Metrics (Prometheus):
  - `governance_dag_blocks_total{kind}`
  - `governance_dag_publish_duration_seconds_bucket`
  - `governance_dag_ipns_updates_total{result}`
  - `governance_dag_validation_failures_total{reason}`
  - `governance_dag_car_queue_depth`
- Logs structured JSON containing `cid`, `parent_cid`, `payload_kind`, `payload_hash`, `status`.
- Grafana dashboards: head timeline, publish latency, block kind distribution, IPFS pin status.
- Alert thresholds:
  - No new block for >12h -> warning.
  - Validation errors >0 in 6h -> critical.
  - IPNS update lag >60s -> critical.

## CLI & API Surface
- `sorafs governance dag head` – show current head (CID, timestamp, parent).
- `sorafs governance dag list --kind advert --limit 50` – list latest blocks of a kind.
- `sorafs governance dag fetch --cid <cid> [--out block.cbor]` – download & verify block.
- `sorafs governance dag export --since 2025-01-01 --out export.car` – produce CAR snapshot.
- `sorafs governance dag diff --from <cid1> --to <cid2>` – show difference summary (new payloads, updated policies).
- REST API (Dashboard backend):
  - `GET /v2/dag/head`
  - `GET /v2/dag/blocks?kind=&after=`
  - `GET /v2/dag/block/{cid}`
  - `GET /v2/dag/snapshots`
  - `GET /v2/dag/proofs?manifest_cid=&provider_id=`

## Rollout Plan
1. Implement ingest/builder/publisher as separate services (Kubernetes deployments). 
2. Deploy to staging with dedicated IPFS Cluster, run shadow mode (no IPNS updates) until validation passes.
3. Enable IPNS publishing in staging; integrate dashboards.
4. Production rollout:
   - Stage 0: read-only (no IPNS updates), verifying blocks.
   - Stage 1: start IPNS updates, monitor metrics.
   - Stage 2: enforce pipeline gating—governance proposals rejected if DAG publish fails.
5. Notify ecosystem teams; update docs portal with fetch/verify instructions.

## Testing Strategy
- Unit tests cover block creation, signature verification, error conditions.
- Integration tests spin up local IPFS cluster (3 `ipfs-http-client` containers) verifying pinning & IPNS updates.
- End-to-end tests replay fixture directories (`fixtures/sorafs_manifest/governance/`, `por/`, `repair/`) and assert CLI verification success.
- Chaos tests simulate IPFS node failures; pipeline must retry pins and alert.
- Snapshot tests ensure `export` + `import` produce identical block hashes.

## Documentation & Tooling
- Operator guide: `docs/source/sorafs/governance_dag_operator.md` describing pipeline operation, incident response.
- Dashboard playbook: `docs/source/sorafs/governance_dag_metrics.md` mapping metrics to alarms.
- Developer guide: `docs/portal/docs/sorafs/governance-dag.md` for SDKs, includes IPNS usage examples.
- CLI cookbook under `docs/examples/sorafs_governance_dag/`.
- Taikai cache profiles: JSON sources live under `configs/taikai_cache/` and `cargo xtask sorafs-taikai-cache-bundle` emits the governance bundle (`profile.json`, `cache_config.json`, Norito `.to`, manifest, and the global `artifacts/taikai_cache/index.json`) used by SNNet-14 rollouts.

## Implementation Checklist
- [x] Document architecture (ingest, builder, publisher, mirror, CLI).
- [x] Define `GovernanceDagNodeV1` schema, block creation, and signature strategy.
- [x] Specify publishing workflow with IPFS Cluster + IPNS.
- [x] Detail retention, snapshotting, and cold storage.
- [x] Capture security, validation, and alerting requirements.
- [x] Outline CLI/API surfaces and rollout plan.
- [x] Describe testing, documentation, and operator tooling expectations.

With this specification, teams can implement the governance DAG pipeline confidently, ensuring deterministic publication, verifiable history, and robust observability for all governance artefacts.
