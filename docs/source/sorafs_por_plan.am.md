---
lang: am
direction: ltr
source: docs/source/sorafs_por_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2d04916bae77ab8682e0e7123e0b781c6720ae7c0232143ce9b4cb75650e7b2
source_last_modified: "2026-07-06T19:43:41.006556+00:00"
translation_last_reviewed: 2026-07-05
source_mtime: "2026-07-06T19:43:41.006556+00:00"
---

# SoraFS PoR Challenge Scheduler & Randomness Integration

## Goals & Scope
- Provide a deterministic yet unpredictable challenge pipeline that proves SoraFS providers hold the chunks they advertise.
- Combine public randomness with provider-specific VRF attestations to eliminate bias while keeping challenges reproducible for audits.
- Track the PoR coordinator architecture, Norito payloads, sampling policies, and response handling required by roadmap item **SF-9a — Challenge scheduler & randomness integration**.
- Align PoR results with repair automation (SF-8b) and future proof initiatives (SF-9, SF-13, SF-14).

## Status
The local SF-9a state machine and reporting foundations are implemented. Torii
persists coordinator state to a Norito snapshot, exposes status/export/report
and ingestion endpoints, records ingestion telemetry, and ships dashboard
panels plus alert fixtures. The reference validator also provides
`sorafs-validate por --challenge <path> --proof <path>`.

The former deterministic randomness adapter fabricated bytes labelled as a
drand signature and paired them with an empty VRF source. It has been removed
from production wiring. `torii.sorafs_por.enabled = true` now fails startup
closed until a configured external drand verifier and provider-VRF feed are
implemented; `randomness_seed_hex` is never accepted as authenticated drand.
Consequently, remaining SF-9a work includes that implementation and live
deployment evidence, plus any production governance archive handoff required
by the operator; each deployment's SQL/Parquet archive backend decision is now
part of the checked reporting/archive evidence, and operator-required
governance archive handoff evidence must carry a fingerprinted digest.
`scripts/check_sorafs_por_rollout_evidence.py` now provides
the fail-closed SF-9 rollout evidence gate for deployed PoR scheduler,
randomness, validator, reporting, archive, observability, and governance
promotion packets, and `scripts/run_sorafs_por_rollout_evidence.py` provides
the matching reviewed collection planner/runner. The checker exports its
required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the
planner includes the checker-backed `evidence_contract` map in dry-run output
for the selected required kinds, and validates the schema-closed collection
plan, required kinds, thresholds, external evidence map, evidence contract, and
command steps before dry-run output or verifier execution. The shared runner
plan guard also rejects non-canonical nested required-kind, threshold,
external-evidence, evidence-contract, and command-step shapes before dry-run
output or verifier execution. The gate now also requires
scheduler runtime, validator replay, reporting/archive, observability, and
governance approval artifacts to carry a `seed_replay_digest_hex` matching a
valid randomness artifact in the same evidence bundle. Seed-replay mismatches
are recorded on the offending artifact in the JSON summary before required-kind
validity is reported. Randomness artifacts also carry `policy_digest_hex`,
valid PoR policy digests are published as `valid_policy_digests`, and
governance approval evidence must bind its `policy_digest_hex` to one of those
valid randomness policy digests. Policy mismatches are recorded on the offending
governance approval artifact through the same summary path.
PoR payload-safety artifacts must explicitly set `raw_randomness_included`,
`raw_vrf_included`, `response_bodies_included`,
`raw_challenge_bytes_included`, `raw_proof_bytes_included`,
`raw_report_included`, `raw_export_included`, and `critical_alerts_firing` to
`false` before promotion can report ready.
`scripts/build_sorafs_por_canary.py` builds individual payload-free SF-9 canary
artifacts for randomness, scheduler runtime, validator replay,
reporting/archive, observability, and governance approval evidence. The
builder requires reviewed deployment context, complete runtime/reporting route
and metric coverage where applicable, rejects duplicate or unknown
runtime-route, reporting-route, and metric inputs before writing, seed-replay
digest bindings, provider and challenge minimum counts, reviewed provider names
using lowercase `provider-*` labels without non-production markers whose unique
inventory matches `provider_count`, reviewed `por-challenge-*` challenge labels
without non-production markers whose unique inventory matches `challenge_count`,
per-route `body_blake3_hex` response digest evidence, route, scheduler-lag, and
report-latency threshold facts, the SQL/Parquet
archive backend selection, governance archive handoff digest evidence,
config-backed governance metadata, reviewed
policy digest input for randomness and governance-approval canaries, and
validates every generated artifact through
`scripts/check_sorafs_por_rollout_evidence.py` before writing.
Checked-in response-file examples cover randomness and scheduler-runtime
canaries.

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

All payloads carry Norito headers for canonical decoding. Torii accepts
authenticated provider proofs at `/v1/sorafs/capacity/por-proof` and
trusted-threshold auditor verdicts at `/v1/sorafs/capacity/por-verdict`.
No external challenge-submission route is mounted. The verified coordinator
scheduler is the only permitted production challenge
authority, and PoR automation enablement fails closed until its external
drand/VRF inputs are implemented. Coordinator status surfaces are under
`/v1/sorafs/por/*`.

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
   - Publish scheduler-originated `PorChallengeV1` to providers; public REST mutation is not an authority boundary.
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

## Operational integration (runtime readiness)

- **Coordinator runtime wiring:** `PorCoordinatorRuntime` (see
  `crates/iroha_torii/src/sorafs/por.rs`) exposes `run_once_at`, `run_once`, and
  `spawn`, but Torii does not construct it from unauthenticated entropy.
  `torii.sorafs_por.enabled = true` is rejected at startup until verified
  external drand and provider-VRF adapters are configured. The legacy optional
  `randomness_seed_hex` is deterministic test material and cannot satisfy this
  readiness gate; defaults keep automation disabled.
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

The local SF-9 state/report integration is implemented. Challenge generation
remains release-blocked until verified external drand/VRF feeds are implemented
and configured; live deployment evidence and any production governance archive
handoff remain required.

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
repair/governance handoff, missing `sorafs-validate por` replay, report latency
above threshold, missing PoR metrics
or alerts, critical alerts, seed replay digest drift across runtime/replay/
reporting/observability/governance artifacts, and governance packets not bound
to `iroha_config`. Governance approval evidence must also carry a
`policy_digest_hex` matching a valid randomness artifact. Seed-replay binding
failures are attached to the offending artifact in the emitted summary, and
policy binding failures are attached to the governance approval artifact.
Route latency, scheduler lag, and report latency evidence must be
non-negative integer-unit values before satisfying rollout ceilings.
Randomness artifacts also bind `provider_count` and `challenge_count` to the
unique canonical `providers[].name` and `challenges[].name` inventories and
reject duplicate provider or challenge entries before promotion can report
ready. Provider inventory labels must use reviewed lowercase `provider-*` IDs
without non-production markers, and challenge inventory labels must use reviewed
lowercase `por-challenge-*` IDs without non-production markers.
Scheduler-runtime and reporting/archive artifacts also bind `route_count` to the
unique canonical `routes[].name` inventory and reject duplicate or unknown route
entries before promotion can report ready. Every route response must include a
`body_blake3_hex` digest before runtime or reporting readiness can report ready.
Observability artifacts also bind `metric_count` to the unique canonical
`metrics` inventory, require the reviewed PoR metric set, and reject duplicate
or unknown metric labels before promotion can report ready.
Reporting/archive artifacts fingerprint the reviewed `archive_backend` value and
`governance_archive_handoff_digest_hex`, the summary exports the sorted
reviewed `metrics` inventory plus `metric_count_values`, `archive_backends`,
and `valid_governance_archive_handoff_digests`, and the aggregate
production-readiness gate requires those fields to match the observability and
reporting/archive artifact fingerprints before final promotion can report
ready. The PoR gate fail-closes when more than one valid seed replay, policy,
or governance archive handoff anchor appears, and clears the mixed
`valid_seed_replay_digests`, `valid_policy_digests`, or
`valid_governance_archive_handoff_digests` set before aggregate promotion can
report ready.
Aggregate promotion also rechecks the lane-proven PoR digest relationships:
seed-replay-bound artifact fingerprints must match `valid_seed_replay_digests`,
and policy-bound artifact fingerprints must match `valid_policy_digests` before
final promotion can report ready.

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
  templates, payload-free canary builder, and focused tests, including
  cross-artifact seed replay digest binding with per-artifact summary
  invalidation and dry-run export of the checker-backed evidence contract.
  Randomness provider/challenge inventory binding and policy digest binding from
  randomness to governance approval are now covered by the same evidence gate,
  and reporting/archive evidence now fingerprints the reviewed archive backend
  and governance archive handoff digest.

Remaining production gates:
- Archive a live drand/VRF/auditor run showing deterministic challenge
  generation and verdict replay that passes the SF-9 rollout evidence gate with
  all runtime/replay/reporting/governance evidence bound to the same seed replay
  digest, randomness evidence bound to reviewed provider and challenge
  inventories, governance approval bound to the randomness policy digest, any
  operator-required governance archive handoff carried as
  `governance_archive_handoff_digest_hex`, and any binding failure marked on the
  offending artifact in the emitted summary.
