---
lang: ja
direction: ltr
source: docs/source/sorafs_por_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ab47380ee5f214b20198f21426d47e91566ce9c24f19c26532f39f9baf2af538
source_last_modified: "2026-06-25T17:41:33+00:00"
translation_last_reviewed: 2026-06-25
---

# SoraFS PoR Challenge Scheduler & Randomness Integration

## Goals & Scope
- Provide a deterministic yet unpredictable challenge pipeline that proves SoraFS providers hold the chunks they advertise.
- Combine public randomness with provider-specific VRF attestations to eliminate bias while keeping challenges reproducible for audits.
- Track the PoR coordinator architecture, Norito payloads, sampling policies, and response handling required by roadmap item **SF-9a — Challenge scheduler & randomness integration**.
- Align PoR results with repair automation (SF-8b) and future proof initiatives (SF-9, SF-13, SF-14).

## Status
The local SF-9a scheduler and reporting foundations are implemented. Torii builds
`PorCoordinatorRuntime` from `torii.sorafs_por`, persists coordinator state to a
Norito snapshot when configured, starts the runtime when both PoR and embedded
SoraFS storage are enabled, exposes status/export/report and ingestion endpoints,
records scheduler/ingestion telemetry, and ships dashboard panels plus alert
fixtures. The reference validator also provides
`sorafs-validate por --challenge <path> --proof <path>`.

Remaining SF-9a rollout work is live deployment evidence for external drand,
VRF, and auditor feeds, plus any production governance archive handoff required
by the operator. `scripts/check_sorafs_por_rollout_evidence.py` now provides
the fail-closed SF-9 rollout evidence gate for deployed PoR scheduler,
randomness, validator, reporting, archive, observability, and governance
promotion packets, and `scripts/run_sorafs_por_rollout_evidence.py` provides
the matching reviewed collection planner/runner. The gate now also requires
scheduler runtime, validator replay, reporting/archive, observability, and
governance approval artifacts to carry a `seed_replay_digest_hex` matching a
valid randomness artifact in the same evidence bundle. Seed-replay mismatches
are recorded on the offending artifact in the JSON summary before required-kind
validity is reported.

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
The canonical schema lives in `sorafs_manifest::por`:

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

All payloads carry Norito headers for canonical decoding. Torii currently accepts
capacity PoR lifecycle submissions at `/v1/sorafs/capacity/por-challenge`,
`/v1/sorafs/capacity/por-proof`, and `/v1/sorafs/capacity/por-verdict`, and
the coordinator status surfaces are under `/v1/sorafs/por/*`.

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
- `sorafs_manifest::reference::validate_por_challenge_proof_bytes` validates
  canonical Norito decoding, `PorChallengeV1` and `PorProofV1` structural
  policy, challenge/proof binding, deadline policy, and exact sample-index
  coverage.
- Torii records challenges, proofs, and auditor verdicts through the capacity
  PoR submission routes and exposes the resulting history through the
  coordinator status/export/report endpoints.
- Full live auditor verification against external drand/VRF feeds and any
  deployment-specific Merkle replay archive remain rollout evidence items.

## Telemetry & Alerts
- Implemented Torii runtime metrics (Prometheus):
  - `torii_sorafs_por_challenges_total{result}` — `scheduled`, `forced`, or `failed`.
  - `torii_sorafs_por_forced_challenges_total`.
  - `torii_sorafs_por_sampling_duplicates_total`.
  - `torii_sorafs_por_ingest_backlog{manifest,provider}`.
  - `torii_sorafs_por_ingest_failures_total{manifest,provider}`.
- Deployed auditor/drand/VRF integration metrics to add with live rollout evidence:
  - `sorafs_por_response_latency_seconds_bucket{result}`.
  - `sorafs_vrf_missing_total`.
  - `sorafs_por_seed_verification_failures_total{reason}`.
- Implemented alerts:
  - `SoraFSPoRSchedulerFailures`: any failed scheduler tick over 15 minutes.
  - `SoraFSPoRForcedChallenges`: any forced challenge over 2 hours.
  - `SoraFSPoRIngestBacklogHigh`: backlog above 3 items for 10 minutes.
  - `SoraFSPoRDuplicateSamplesHigh`: more than 100 duplicate samples in 1 hour.
- Logs include `epoch_id`, `manifest_digest`, `provider_id`, `sample_count`, `result`, `failure_reason`.
- `dashboards/grafana/sorafs_gateway_observability.json` overlays gateway proof
  outcomes with PoR scheduler and ingestion health.

## Persistence
Current local persistence is `PorCoordinator::with_persistence`, which snapshots
coordinator state to a Norito file such as `por_coordinator_snapshot.norito`
under the configured storage directory. The SQL shape below remains the
production warehouse/archive target for operators that need long-retention
analytics outside the node snapshot.

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

## Operational integration (runtime wired)

- **Coordinator runtime wiring:** `PorCoordinatorRuntime` (see
  `crates/iroha_torii/src/sorafs/por.rs`) exposes `run_once_at`, `run_once`, and
  `spawn`. Torii builds it from `Config::sorafs_por` and starts it during Torii
  startup when `torii.sorafs_por.enabled` is true and embedded SoraFS storage is enabled.
  The config supplies `epoch_interval_secs`, `response_window_secs`, `governance_dag_dir`,
  and optional `randomness_seed`; defaults keep the runtime disabled until operators opt in.
- **Storage hooks:** The runtime uses `sorafs_node::NodeHandle` as its `PorStorage`, plans
  challenges from the local manifest/capacity state, records accepted challenges, and leaves
  proof/verdict persistence to the existing Torii PoR submission routes. The ingestion status
  endpoint (`GET /v1/sorafs/por/ingestion/{manifest_digest_hex}?limit=N`) reports
  backlog depth, oldest epoch/deadline, and last success/failure timestamps with
  `limit`-bounded provider status entries and total provider counts.
- **Governance events:** Published challenges and weekly reports are materialised by
  `FilesystemGovernancePublisher` under the configured governance DAG directory. Status,
  export, and report endpoints expose the coordinator history as canonical Norito payloads.
- **Alerts:** `dashboards/alerts/sorafs_por_rules.yml` covers scheduler failures, forced
  challenges, ingestion backlog, and duplicate sample spikes.

Implementation status: Torii exposes `/v1/sorafs/por/ingestion/{manifest_digest_hex}?limit=N`,
which delegates to `sorafs_node::NodeHandle::por_ingestion_status` for backlog
depth, oldest epoch/deadline, and last verdict timestamps while bounding the
returned provider status array. A dedicated sampler (`SharedAppState::spawn_por_ingestion_metrics_worker`)
collects `por_ingestion_overview` snapshots every 30 seconds and drives the
`torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` gauges so dashboards and
alerts stay fresh even when providers are idle; stale providers are zeroed out whenever they drop
from the snapshot.【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/lib.rs:7859】【crates/iroha_telemetry/src/metrics.rs:10452】

The local SF-9 runtime integration is implemented. Remaining rollout work is live
deployment evidence for external drand/VRF/auditor feeds and any production governance
archive handoff required by the deployment operator.

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

The rollout evidence scripts have focused Python coverage in:

- `scripts/tests/check_sorafs_por_rollout_evidence_test.py`
- `scripts/tests/run_sorafs_por_rollout_evidence_test.py`

## Rollout Evidence Gate

Operators should keep SF-9 promotion fail-closed until the payload-free
deployment evidence passes the checked-in gate:

```bash
python3 scripts/check_sorafs_por_rollout_evidence.py \
  @scripts/examples/sorafs_por_rollout_evidence.args.example
```

For reviewed collection planning, use the runner in dry-run mode before
executing it against captured evidence paths:

```bash
python3 scripts/run_sorafs_por_rollout_evidence.py \
  @scripts/examples/sorafs_por_rollout_collection.args.example \
  --dry-run
```

The checker recognizes `sorafs.por.*` SF-9 rollout schemas for randomness,
scheduler runtime, validator replay, reporting/archive handoff, observability,
and governance approval. It fails closed on stale evidence, raw challenge/proof,
drand, VRF, report, export, response-body, transaction, token, secret, and key
material, under-sized provider or challenge samples, unauthenticated or
non-Norito routes, route latency above threshold, scheduler lag above threshold,
missing deterministic seed replay, missing drand/VRF validation, missing
repair/governance handoff, missing `sorafs-validate por` replay, unresolved
manual-trigger route policy, report latency above threshold, missing PoR metrics
or alerts, critical alerts, seed replay digest drift across runtime/replay/
reporting/observability/governance artifacts, and governance packets not bound
to `iroha_config`. Seed-replay binding failures are attached to the offending
artifact in the emitted summary.

## Rollout Status
Implemented locally:
- `sorafs_manifest::por` challenge, proof, status, manual challenge, provider
  summary, slashing event, and weekly-report payloads.
- `PorCoordinator`, `PorCoordinatorRuntime`, optional Norito snapshot
  persistence, filesystem governance publishing, and Torii startup wiring.
- Capacity PoR submission routes plus `/v1/sorafs/por/status`,
  `/v1/sorafs/por/export`, `/v1/sorafs/por/report/{iso_week}`, and
  `/v1/sorafs/por/ingestion/{manifest_digest_hex}`.
- Scheduler, forced-challenge, duplicate-sample, and ingestion telemetry with
  checked-in dashboard and alert fixtures.
- `generate_por_fixtures` and `sorafs-validate por` reference validation.
- Fail-closed SF-9 rollout evidence gate, collection planner, operator argfile
  templates, and focused tests, including cross-artifact seed replay digest
  binding with per-artifact summary invalidation.

Remaining production gates:
- Archive a live drand/VRF/auditor run showing deterministic challenge
  generation and verdict replay that passes the SF-9 rollout evidence gate with
  all runtime/replay/reporting/governance evidence bound to the same seed replay
  digest and any binding failure marked on the offending artifact in the emitted
  summary.
- Decide whether each deployment needs the SQL/Parquet warehouse layer in
  addition to the node-local Norito snapshot.
- Capture governance DAG archive handoff evidence for production operators and
  include it in the SF-9 reporting/archive evidence packet.
