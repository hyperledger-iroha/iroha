---
lang: kk
direction: ltr
source: docs/source/sorafs_por_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 876a4d8556515611ca194aac7208bf2cddf1de606464bcbf8f5d738f4ed438e1
source_last_modified: "2025-12-29T18:16:36.173773+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS PoR Challenge Scheduler & Randomness Integration
summary: Final specification for SF-9a covering randomness, sampling, coordinator workflow, telemetry, and persistence.
---

# SoraFS PoR Challenge Scheduler & Randomness Integration

## Goals & Scope
- Provide a deterministic yet unpredictable challenge pipeline that proves SoraFS providers hold the chunks they advertise.
- Combine public randomness with provider-specific VRF attestations to eliminate bias while keeping challenges reproducible for audits.
- Define the PoR coordinator architecture, Norito payloads, sampling policies, and response handling required by roadmap item **SF-9a — Challenge scheduler & randomness integration**.
- Align PoR results with repair automation (SF-8b) and future proof initiatives (SF-9, SF-13, SF-14).

This document completes SF-9a, promotes the scheduler blueprint from draft to implementation-ready, and keeps SF-9 overall in-flight until validator tooling (SF-9b) lands.

## Randomness Model
1. **Epoch cadence:** 1-hour epochs (`epoch_id = floor(unix_time / 3600)`).
2. **Public entropy:** Latest drand round (`drand_round`, `drand_signature`, `drand_randomness`). Torii fetches using TLS with pinned CA; rounds must be within 2 minutes of epoch start.
3. **Provider VRF:** Each provider signs the current `epoch_id` with its registered VRF key. Payload:
   ```
   vrf_input = norito::json::to_vec({
       "epoch_id": epoch_id,
       "provider_id": provider_id,
       "manifest_digest": manifest_digest
   })
   ```
   Resulting `vrf_output` and `vrf_proof` are submitted via `ProviderAdvert` updates. Failure to supply a fresh VRF proof for an epoch marks the provider `ineligible` and triggers governance alerts.
4. **Seed derivation:** `seed = BLAKE3(drand_randomness || vrf_output || manifest_digest || epoch_id_le)`. Seeds are 32 bytes.
5. **Deterministic RNG:** Use `ChaCha20Rng::from_seed(seed)` (Rust `rand_chacha`) to sample chunk indices.
6. **Bias resistance:** If a provider omits VRF, the coordinator substitutes a zero vector and marks the challenge as `forced`. Forced challenges count towards failure metrics even if proofs succeed, incentivising timely VRFs.

## Sampling Policy
- Sampling honours content-defined chunking (CDC) metadata embedded in manifests.
- Two leaf granularities:
  - **4 KiB blocks**: `leaf_kind = Small`.
  - **64 KiB blocks**: `leaf_kind = Large`.
- Mixed manifests include both leaf kinds; sampling ratio ensures representation of each.

| Profile Tier | Manifest Size | Leaf Mix | Samples per Epoch | Coverage Guarantees |
|--------------|---------------|----------|-------------------|---------------------|
| T1 — Edge | <10 GiB | 100% 4 KiB | 64 | ≥0.25% of leaves |
| T2 — Standard | 10–100 GiB | 75% 4 KiB / 25% 64 KiB | 128 (96 small, 32 large) | ≥0.25% small leaves, ≥0.1% large leaves |
| T3 — Archival | >100 GiB | 100% 64 KiB | 256 | ≥0.2% of leaves |

- Governance overrides (`profile.sample_multiplier`) scale sample counts by integer factors (1–4).
- Scheduler tracks per-manifest sample state to avoid duplicates within an epoch; if RNG produces a previously sampled index, draw again until a fresh value appears or 8 attempts fail (after which duplicates are allowed but flagged in telemetry).

## Norito Payloads
New schema (to live in `sorafs_manifest::por`):

```norito
struct PorChallengeV1 {
    manifest_digest: Digest32,
    provider_id: ProviderId,
    epoch_id: U64,
    drand_round: U64,
    drand_signature: Vec<u8>,
    seed: Digest32,
    sample_tier: PorSampleTier,
    samples: Vec<PorSampleV1>,
    response_deadline_unix: U64,
}

struct PorSampleV1 {
    leaf_kind: PorLeafKind,          // small | large
    leaf_index: U64,
    chunk_offset: U32,               // byte offset within leaf (0 for 64 KiB)
    chunk_length: U32,
    blake3_digest: Digest32,
}

struct PorProofV1 {
    manifest_digest: Digest32,
    provider_id: ProviderId,
    epoch_id: U64,
    challenge_seed: Digest32,
    samples: Vec<PorSampleProofV1>,
    signature: Signature,
}

struct PorSampleProofV1 {
    leaf_kind: PorLeafKind,
    leaf_index: U64,
    chunk_offset: U32,
    payload: Vec<u8>,                // actual chunk bytes
    merkle_path: Vec<ProofNodeV1>,
}
```

All payloads carry Norito headers for canonical decoding. `PorChallengeV1` is broadcast via Torii `/sorafs/por/challenge` and enqueued to provider queues. Providers respond with `PorProofV1` within the response window (see below).

## Coordinator Workflow
1. **Epoch bootstrap**
   - Fetch drand round; verify BLS signature.
   - Collect provider VRF proofs for active manifests.
   - For manifests without VRF, mark `forced`.
2. **Sample generation**
   - Determine sample tier from manifest metadata (`profile.sample_tier`).
   - Seed RNG and produce `samples`.
   - Persist pending challenge (see Persistence).
3. **Challenge dispatch**
   - Publish `PorChallengeV1` via Torii (REST + WebSocket).
   - Write event to Governance DAG (`governance/sorafs/por/challenges/<epoch_id>/<manifest_...>.json`).
   - Response deadline = `epoch_start + 15 minutes`.
4. **Proof handling**
   - Providers submit `PorProofV1` via `POST /sorafs/por/proof`.
   - Coordinator verifies proof (Merkle paths, digests, manifest alignment).
   - On success: update history to `verified`, emit telemetry, and sign `AuditVerdictV1` summarising result.
   - On failure or timeout: mark `failed`, emit `PorFailureEventV1`, hand over to repair scheduler (SF-8b).
5. **Retry logic**
   - If submission fails due to transport, provider may retry until deadline; the coordinator keeps the earliest valid proof.
   - After deadline, coordinator optionally issues a `grace` challenge (smaller sample set) if network anomalies are detected; otherwise escalate immediately.

## Proof Verification
- Reuse `sorafs_manifest::proof_stream::Verifier`.
- Steps:
  1. Validate challenge seed matches recomputed seed (drand + VRF). Reject if mismatch.
  2. Verify each `PorSampleProofV1`:  
     - Merkle path leads to manifest root.  
     - Chunk bytes hashed with `blake3` equals `PorSampleV1::blake3_digest`.  
     - `chunk_length` matches manifest chunk metadata.
  3. Ensure provider signature covers the entire proof payload.
  4. Audit log includes `verification_time_ms`, `chunk_bytes_total`.
- Failures categorised as: `seed_mismatch`, `merkle_invalid`, `digest_mismatch`, `expired`, `signature_invalid`.

## Telemetry & Alerts
- Metrics (Prometheus):
  - `sorafs_por_challenges_total{result}` — success/failed/forced.
  - `sorafs_por_response_latency_seconds_bucket{result}`.
  - `sorafs_vrf_missing_total`.
  - `sorafs_por_sampling_duplicates_total`.
  - `sorafs_por_seed_verification_failures_total{reason}`.
- Alerts:
  - `SORAfsPorFailuresHigh`: failure rate >5% over 6 hours.
  - `SORAfsVrfMissing`: provider missing VRF for 3 consecutive epochs.
  - `SORAfsPorUnverified`: >20 pending challenges older than deadline.
- Logs include `epoch_id`, `manifest_digest`, `provider_id`, `sample_count`, `result`, `failure_reason`.
- Grafana dashboards overlay PoR outcomes with repair queue depth to visualise churn.

## Persistence
```sql
CREATE TABLE sorafs_por_history (
    id BIGSERIAL PRIMARY KEY,
    manifest_digest BYTEA NOT NULL,
    provider_id BYTEA NOT NULL,
    epoch_id BIGINT NOT NULL,
    drand_round BIGINT NOT NULL,
    seed BYTEA NOT NULL,
    sample_tier SMALLINT NOT NULL,
    sample_count INTEGER NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL,
    deadline_at TIMESTAMPTZ NOT NULL,
    responded_at TIMESTAMPTZ,
    status SMALLINT NOT NULL, -- 0 pending,1 verified,2 failed,3 repaired,4 forced
    failure_reason TEXT,
    proof_digest BYTEA,
    repair_task_id UUID,
    gov_event_cid TEXT
);

CREATE TABLE sorafs_vrf_history (
    provider_id BYTEA NOT NULL,
    manifest_digest BYTEA NOT NULL,
    epoch_id BIGINT NOT NULL,
    vrf_output BYTEA NOT NULL,
    vrf_proof BYTEA NOT NULL,
    received_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (provider_id, manifest_digest, epoch_id)
);
```

- Retention: 180 days hot; nightly compactor exports Parquet to `s3://sorafs-audit/por/YYYY/MM/DD`.
- `proof_digest` stores SHA-256 of `PorProofV1` saved in object storage for later audits.
- `gov_event_cid` references the DAG entry containing the public verdict.

## Operational integration (runtime work-in-progress)

- **Coordinator runtime wiring:** `PorCoordinatorRuntime` (see
  `crates/iroha_torii/src/sorafs/por.rs`) now exposes `PorCoordinatorRuntime::drive_epoch`.
  Thread this into the Torii scheduler loop so challenges are issued even when no HTTP traffic is
  present. The runtime should honour `por.coordinator.{enabled,interval_secs}` config knobs.
- **Storage hooks:** The SoraFS node (`crates/sorafs_node`) consumes coordinator events via a
  channel so proofs are streamed straight into the persistent store. A new ingestion status endpoint
  (`GET /sorafs/por/ingestion/{manifest}`) reports backlog depth and last success timestamp.
- **Governance events:** Every verified or failed proof results in a Norito event emitted through
  `GovernancePublisher`, tagged with `por_epoch`, `manifest_digest`, and `provider_id`, so
  GovernanceLog subscribers can correlate verdicts with repair automation.
- **Alerts:** Feed `sorafs_por_ingest_backlog`, `sorafs_por_ingest_failures_total`, and
  `sorafs_por_forced_challenges_total` metrics into the existing dashboards. Alert when backlog
  exceeds 3 epochs or forced challenges persist for >2 epochs.

Implementation status: Torii exposes `/v1/sorafs/por/ingestion/{manifest_digest_hex}`, which
delegates to `sorafs_node::NodeHandle::por_ingestion_status` for backlog depth, oldest epoch/deadline,
and last verdict timestamps. A dedicated sampler (`SharedAppState::spawn_por_ingestion_metrics_worker`)
collects `por_ingestion_overview` snapshots every 30 seconds and drives the
`torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` gauges so dashboards and
alerts stay fresh even when providers are idle; stale providers are zeroed out whenever they drop
from the snapshot.【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/lib.rs:7859】【crates/iroha_telemetry/src/metrics.rs:10452】

Implementation of these hooks is the remaining SF‑9 deliverable referenced by `roadmap.md`
("PoR coordinator runtime integration").

## Integration with Repair Automation
- On `failed` status, coordinator emits:
  ```norito
  struct PorFailureEventV1 {
      manifest_digest: Digest32,
      provider_id: ProviderId,
      epoch_id: U64,
      failure_reason: PorFailureReason,
      proof_digest: Option<Digest32>,
  }
  ```
- Repair scheduler receives the event, attaches to `RepairTaskV1.evidence`.
- After repair success, scheduler updates `sorafs_por_history.status = 3 (repaired)` and `notes`.
- Slash proposals reference the original `PorChallengeV1` and `PorProofV1`.

## Fixtures & QA
- Deterministic fixtures live in `fixtures/sorafs_manifest/por/epoch_<id>/`.
- Generator CLI (`cargo run -p sorafs_manifest --bin generate_por_fixtures`) accepts arguments:
  - `--epoch-id`, `--manifest`, `--sample-tier`, `--seed`.
- Unit tests cover:
  - Seed recomputation vs stored values.
  - Sampling reproducibility across platforms (x86_64, aarch64).
  - Fixture replay verifying `PorProofV1`.
- Trybuild UI tests ensure compile-time errors for malformed Norito payloads.

## Implementation Checklist
- [x] Document randomness combination (drand + VRF) and seed derivation.
- [x] Define sampling tiers, coverage guarantees, and duplicate handling.
- [x] Specify Norito payloads for challenges, proofs, and failure events.
- [x] Describe coordinator workflow, response deadlines, and retries.
- [x] Detail verification steps, metrics, alerts, and logging.
- [x] Provide persistence schema, retention, and DAG integration.
- [x] Outline fixtures/tests ensuring deterministic behaviour.

Remaining work tracked under SF-9 (implementation + validator tooling). This specification, alongside SF-8b’s repair automation plan, enables engineers to build the PoR coordinator without ambiguity.
