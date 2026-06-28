---
lang: az
direction: ltr
source: docs/source/sorafs_indexer_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0a8a3d1d36a0a2b37c6d9f2add236a38ca46ebd3e80773058f1be760be968c8b
source_last_modified: "2026-06-25T16:58:37+00:00"
translation_last_reviewed: 2026-02-07
title: Sora Network Indexer & Delegated Routing Plan
summary: SFM-1 indexer architecture, implemented provider-discovery baseline, and future delegated-routing rollout.
---

# Sora Network Indexer Plan

> **Status (Jun 2026):** The local provider-discovery baseline is implemented:
> Torii ingests signed `ProviderAdvertV1` payloads at
> `/v1/sorafs/providers/advert`, serves the current in-memory advert cache at
> `/v1/sorafs/providers`, exposes configured gateway/pin Torii peers at
> `/v1/sorafs/storage/peers`, bounds both local readback arrays with `limit`,
> validates chunk-range capability metadata, and exports
> range-capability telemetry. The IPNI-compatible `/routing/v1/*` delegated
> routing indexer described below remains a future service/deployment track, not
> a local Torii endpoint available today.

## Objectives
- Mirror IPNI/delegated routing v1 API.
- Index provider adverts, PDP/PoTR scores.
- Co-locate indexer at gateways for low-latency lookups.

## Components
- Implemented baseline: Torii provider advert ingest/list endpoints,
  in-memory TTL pruning, capability validation, range-capability metrics, and
  `limit`-bounded provider/configured-peer list readback with full counts plus
  returned-count/truncation metadata.
- Future service: governance-DAG ingest pipeline for adverts/proofs.
- Future service: API endpoints (`/routing/v1/find`, etc.).
- Future service: durable caching layer and regional TTL policies.
- Future service: metrics for delegated-routing query volume and freshness.

## Data Schema & Storage Strategy

The indexer stores a denormalised view of provider metadata that mirrors the canonical
Norito payloads coming from the governance DAG. The storage stack is split into three
layers so queries remain low-latency while preserving the audit trail:

1. **Hot KV cache (RocksDB or TiKV).** Holds the latest `ProviderAdvertV1`,
   `ProviderAdmissionEnvelopeV1`, PoTR/PoR score snapshots, and capacity declarations keyed
   by provider ID. Each record is the raw Norito payload plus a compact JSON projection
   consumed by the API. Keys follow the pattern `provider:<id>:advert@v1`,
   `provider:<id>:por_score@<cycle>`, etc. TTLs mirror advert expiry and PoTR windows.
2. **Relational store (PostgreSQL + TimescaleDB).** Persists historical records for auditing
   and analytics. Tables:
   - `provider_adverts(provider_id, version, issued_at, expires_at, qos, capabilities, endpoints, stream_budget)`
   - `routing_scores(provider_id, score_kind, score_value, sample_window_start, sample_window_end)`
   - `por_challenges(challenge_id, provider_id, manifest_cid, outcome, sample_count, timestamp)`
   - `governance_signatures(record_id, hash, council_member, signature_bytes)`
   JSONB columns retain the original Norito payload; indexed projections (GIN on capabilities,
   BRIN on timestamps) support delegated routing queries.
3. **Object storage (CAR/IPLD).** Archival of every DAG block fetched during ingestion.
   Stored under `ipfs://sorafs-indexer/<cycle>/governance.car` so auditors can recompute the
   SQL/KV state. The indexer records CAR CIDs in the relational store to make backfills deterministic.

Ingest pipeline sequence:

1. Consume governance DAG events (`ProviderAdvertNode`, `ReplicationOrderNode`, `PorProofNode`,
   etc.) by decoding the Norito payloads defined in `sorafs_manifest`.
2. Validate signatures/hashes, write the canonical payload to object storage, then apply the
   latest record into RocksDB (hot path) and append to PostgreSQL (historical path) inside a
   single transactional boundary.
3. Update materialised views (`provider_capability_index`, `provider_latency_view`) that feed
   the `/routing/v1/find` query planner.

Caching rules:
- Hot cache entries expire when `expires_at` crosses current time or when a superseding advert
  arrives. A background reconciler scans PostgreSQL for stale cache entries every 5 minutes.
- Capability lookups use composite cache keys (`capability:<cap_type>:<region>`) precomputed
  during ingest for O(1) API access.

## Deployment Strategy

The indexer runs as a horizontally scalable microservice with regional shards to keep latency
low for clients colocated with gateways.

- **Topology.**
  - A **global ingest cluster** (three nodes) follows the governance DAG and writes to the
    authoritative PostgreSQL + object storage in the primary region (e.g., `eu-central-1`).
  - **Regional read replicas** (one per gateway region) run read-only PostgreSQL replicas +
    RocksDB caches. They subscribe to a Kafka/Redpanda topic fed by the ingest cluster so hot
    updates propagate within <2 seconds.
- **Placement.** Each Torii gateway hosts a co-located indexer pod that fronts the regional
  cache and exposes the `/routing/v1/*` API. Requests fall back to the primary region when the
  local replica lags beyond 30 seconds (measured via replication LSN monitoring).
- **Scaling.**
  - Autoscale reader pods based on QPS and cache hit rate (target >95% hits).
  - Schedule nightly compaction jobs for RocksDB and run PostgreSQL VACUUM after each weekly
    governance cycle.
- **Resilience.**
  - Cross-region object storage replication ensures CAR archives survive regional failures.
  - Disaster recovery playbooks restore RocksDB caches from PostgreSQL snapshots by replaying
    the latest governance cycle and PoTR score updates.
- **Observability.**
  - Expose `sorafs_indexer_ingest_lag_seconds`, `sorafs_indexer_cache_hit_ratio`,
    `sorafs_indexer_query_latency_seconds`, and `sorafs_indexer_replica_lag_seconds`.
  - Alerts fire when ingest lag exceeds 10 seconds, when any region’s cache hit ratio dips
    below 90%, or when RocksDB compaction backlog exceeds 5 GB.

Future rollout checklist:

1. Deploy ingest cluster + PostgreSQL primary in staging; validate end-to-end sync from DAG.
2. Bring up two regional replicas, simulate adverts/PoR updates, and verify delegated routing
   CLI returns consistent results across regions.
3. Enable production caches, enforce runbooks for failover, and update `status.md` / roadmap
   once SLA metrics stay green for two consecutive governance cycles.
