---
title: SoraFS PoR Challenge Scheduler & Randomness Integration
summary: SF-9a implementation status for PoR randomness, scheduler runtime wiring, telemetry, persistence, and remaining rollout evidence.
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
from production wiring. Torii now constructs a verified randomness provider
from pinned chain public-key/genesis/period metadata, at least three canonical
HTTPS endpoints, a strict-majority quorum, bounded DNS/body/timeouts, freshness
limits, and a durable high-water state file. It verifies unchained G1/RFC 9380
drand signatures, rejects rollback and equivocation, and only returns a round
after the configured endpoint quorum agrees. Provider VRF submissions are
signature-checked, bound to provider/manifest/epoch/drand inputs, persisted
with replay state, and supplied to the coordinator through the verified feed.
`sorafs.por.enabled = true` fails startup when this configuration is
missing or internally inconsistent. It also requires the complete non-secret
`[sorafs.por.potr_runtime]` binding and injected
`PotrRuntimeSignerRolesV1`; every configured signer, qualification, gateway
key, reader/source/resolver identity, and baseline finalized admission field
must match the injected roles exactly. The provider qualification is the
baseline admission sequence/digest. Partial or disabled-stale bindings,
test-marked/shared handles, identity collisions, and injected roles without
enabled configuration fail closed. `randomness_seed_hex` is never accepted as
authenticated drand. The exact configuration/startup binding is
source-complete, while focused/workspace Cargo validation remains pending.
Weekly governance reporting now prepares the previous completed ISO week,
uses that cycle's exact end boundary for `generated_at`, and persists both the
canonical report and its publication acknowledgement before advancing.
Publication failure, process restart, and a multi-week outage therefore retry
the same report bytes and catch up one unskipped ISO week at a time. The
updated hard-cut coordinator snapshot intentionally has no compatibility
decoder for the former pre-release layout. Focused/workspace validation of
this newest report-state change remains pending.
Remaining SF-9a work is live multi-provider drand/VRF/auditor evidence and any
production governance archive handoff required by the operator, not
implementation of the local verified feeds. Each deployment's
SQL/Parquet archive backend decision is part of the checked reporting/archive
evidence, and operator-required governance archive handoff evidence must carry
a fingerprinted digest.
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
the automated coordinator state, config-backed governance metadata, reviewed
policy digest input for randomness and governance-approval canaries, and
validates every generated artifact through
`scripts/check_sorafs_por_rollout_evidence.py` before writing.
Checked-in response-file examples cover randomness and scheduler-runtime
canaries.

## Randomness Model
1. **Epoch cadence:** 1-hour epochs (`epoch_id = floor(unix_time / 3600)`).
2. **Public entropy:** Latest drand round (`drand_round`, `drand_signature`,
   `drand_randomness`). Torii fetches from canonical HTTPS endpoints, verifies
   the BLS signature against pinned chain metadata, requires strict-majority
   endpoint agreement, applies configured freshness/skew bounds, and persists a
   rollback/equivocation high-water mark.
3. **Provider VRF:** Each provider signs the current `epoch_id` with its registered VRF key. Payload:
   ```
   vrf_input = norito::json::to_vec({
       "epoch_id": epoch_id,
       "provider_id": provider_id,
       "manifest_digest": manifest_digest
   })
   ```
   Resulting `vrf_output` and `vrf_proof` are submitted through the authenticated,
   rate-limited `POST /v1/sorafs/por/vrf` route. Torii verifies the registered
   provider key, chain id, manifest, epoch, and drand-round binding before
   durable insertion. Failure to supply a fresh VRF proof before the governed
   deadline triggers the forced-challenge policy and governance telemetry.
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
V1 validators perform an exact whole-payload sizing preflight before nested
work and fail closed when an exact size is unavailable. Canonical ceilings are
64 KiB for a challenge, 128 KiB for a challenge publication, 16 MiB for a
proof, 256 KiB for an audit verdict, 16 KiB for one status, 32 MiB for a weekly
report, and 4 KiB for one provider VRF submission. Proofs are limited to 500
samples with at most 64 authentication-path nodes per sample; Ed25519 public
keys and signatures have exact 32- and 64-byte lengths. Torii bounds the proof
and verdict JSON wrappers to the exact padded-base64 ceiling plus their fixed
field overhead before base64 decoding, and then uses the typed bounded
canonical decoders. The coordinator's 64 MiB persisted snapshot input also has
an absolute 512 MiB cumulative decode-allocation ceiling.

Status material is outcome-specific: `Pending` carries no response or outcome
material; `Verified` carries the proof digest and response timestamp;
`Failed` requires a canonical failure reason and native repair task but may be
proofless; and `Repaired` retains both the failure reason and proof response
while carrying no repair task. A forced challenge without a verdict remains
`Forced` even after a proof arrives. Weekly slashing events are strictly
ordered by `(decided_at, provider_id, manifest_digest, verdict_cid)` and
duplicates are rejected.
No external challenge-submission route is mounted. The verified coordinator
scheduler is the only permitted production challenge
authority, and PoR automation enablement fails closed until its external
drand/VRF inputs and the embedded node's runtime-signed Governance DAG
publisher are ready. Coordinator status surfaces are under
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
   - Construct a validated `PorChallengePublicationV1` containing the canonical
     challenge and its exact bounded duplicate-sample count, then enqueue its
     header-bearing Norito bytes through the embedded node's durable Governance
     DAG outbox. The node writes the signed DAG block/head; no independent raw
     JSON publisher is an authority.
   - Response deadline = `epoch_start + 15 minutes`.
4. **Proof handling**
   - Providers submit `PorProofV1` through authenticated `POST /v1/sorafs/capacity/por-proof`.
   - Coordinator verifies proof (Merkle paths, digests, manifest alignment).
   - Trusted auditors submit signed `AuditVerdictV1` decisions through authenticated `POST /v1/sorafs/capacity/por-verdict`.
   - A failed verdict finalises only after the durable native repair transaction forwarder accepts its canonical task; successful and repaired verdicts create no repair task.
5. **Retry logic**
   - If submission fails due to transport, provider may retry until deadline; the coordinator keeps the earliest valid proof.
   - Exact failed-verdict replay reuses the retained native task identifier without invoking the repair handoff again; a different terminal verdict for the same challenge is rejected.

## Finalized replay archive and reputation handoff

Finalized verdicts now retain a canonical, sequence-bound PoR terminal outcome
until the committed reputation runtime returns a durable exact-replay-aware
admission result. The node checkpoints the matching sequence and work digest
before it becomes eligible for compaction. A supervised `irohad` worker bounds
both reconciliation and compaction per tick, and always performs reputation
admission before archive removal. While the configured reputation runtime is
still assembling its deployment-owned dependencies, its explicit `Deferred`
state produces a zero-work tick: no admission, acknowledgement, or archive
compaction occurs. Once the runtime becomes `Active`, every gate revalidates the
finalized-query, journal-submitter, threshold-signer, and Governance DAG
bindings; an outage or drift is a hard worker failure and cannot be downgraded
back to deferred.

The optional `[sorafs.storage.por_replay_archive]` binding contains only a
stable production handle, archive identity, non-zero revision, public-policy
digest, Ed25519 verification key, bounded worker settings, and independent
maximum successor-receipt count and canonical successor-proof byte limit.
Runtime-provider slot 46 supplies the deployment-owned immutable archive.
Startup and every challenge, verdict, and compaction operation recheck the exact
handle and signed binding before and after the authenticated operation;
missing, unrequested, test-marked, substituted, stale, or drifting providers
fail closed. Archive receipts sign the canonical record, the retained
reputation-work digest, and the predecessor head.

Every lookup names the caller's exact signed checkpoint head. Presence requires
the canonical record plus a bounded, signed, contiguous successor suffix that
ends at that head. Absence is a separate HSM signature over the challenge id and
that exact head; an unbound `None` result is never accepted. A transport-backed
provider must enforce the configured count and framed-byte ceilings from its
outer envelope before allocating or decoding the suffix; the typed boundary
then enforces the count and canonical decoded-byte bounds again. On restart, a
live-ahead head is accepted only when one bounded proof matches every receipt
to the exact acknowledged contiguous prefix still retained locally. The node
then checkpoints that reconciled head before serving. This also covers a first
external append committed immediately before the local checkpoint; a fresh
state with no retained acknowledgement remains rejected. Rollback, fork,
missing or substituted local intent, missing ancestry, and over-limit proofs
fail startup. An exact append retry must return its original receipt even after
successors exist and must not change the monotonic current head. Compaction
removes local replay state only after an authenticated `current_head` readback
equals the final appended receipt and the provider binding remains unchanged;
a signed receipt that was not installed as the authoritative head rolls the
local mutation back for exact retry.

This closes the local contract, hard-cut configuration, route wiring, and
bounded worker seam. It does not supply archive credentials, an HSM private key,
an immutable storage backend, or deployment evidence. Those remain operator
qualification and rollout requirements.

## Proof Verification
- `sorafs_manifest::reference::validate_por_challenge_proof_bytes` validates
  canonical Norito decoding, `PorChallengeV1` and `PorProofV1` structural
  policy, challenge/proof binding, deadline policy, and exact sample-index
  coverage.
- Torii records challenges, proofs, and auditor verdicts through the capacity
  PoR submission routes and exposes the resulting history through the
  coordinator status/export/report endpoints.
- Full live auditor verification against external drand/VRF feeds plus genuine
  qualification of the deployment-owned immutable replay archive and HSM
  signer remain rollout evidence items.

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
- Logs are payload-free and include only `epoch_id`, `manifest_digest`, `provider_id`, `sample_count`, and `result`; verdict reasons, proof bytes, signatures, and metadata are excluded.
- `dashboards/grafana/sorafs_gateway_observability.json` overlays gateway proof
  outcomes with PoR scheduler and ingestion health.

## Persistence
`sorafs.por.state_dir` is the single private PoR state root. The coordinator
snapshot (`por-coordinator.to`), verified drand high-water
(`drand-high-water.to`), and authenticated provider-VRF replay state
(`provider-vrf-state.to`) are derived beneath it. The obsolete
`governance_dir`, `governance_dag_dir`, and independently configurable drand or
VRF state paths are rejected; this directory never serves as a competing
Governance DAG sink. `PorCoordinator::with_persistence` writes the canonical
Norito coordinator snapshot there. That snapshot retains the exact prepared
weekly report and a durable published flag. A report is persisted before the
Governance DAG call, the acknowledgement is persisted after success, pending
bytes block cycle advancement, and restart catch-up advances through every
missing ISO week in order. This is a V1 hard cut: snapshots using the earlier
pre-release field set fail decoding instead of being guessed or migrated. The
SQL shape below remains the production warehouse/archive target for operators
that need long-retention analytics outside the node snapshot.

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
    repair_task_id BYTEA, -- 32-byte native BLAKE3 task id
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
  `spawn`, and Torii constructs it only from the verified drand and durable
  provider-VRF adapters. `sorafs.por.enabled = true` is rejected at
  startup unless pinned drand chain/endpoints/quorum/state and provider-VRF
  state are configured consistently, the exact
  `[sorafs.por.potr_runtime]` public pins match the injected independent
  gateway/provider signer roles, and the embedded node has a fully bound
  runtime Ed25519 Governance DAG signer/publisher. The checked-in
  [`potr_runtime_binding.toml`](sorafs/snippets/potr_runtime_binding.toml)
  fragment enumerates the public PoTR fields; credentials and private keys
  remain runtime-only. The legacy optional
  `randomness_seed_hex` is deterministic test material and cannot satisfy this
  readiness gate; defaults keep automation disabled.
- **Storage hooks:** The runtime uses `sorafs_node::NodeHandle` as its `PorStorage`, plans
  challenges from the local manifest/capacity state, records accepted challenges, and leaves
  proof/verdict persistence to the existing Torii PoR submission routes. The ingestion status
  endpoint (`GET /v1/sorafs/por/ingestion/{manifest_digest_hex}?limit=N`) reports
  backlog depth, oldest epoch/deadline, and last success/failure timestamps with
  `limit`-bounded provider status entries and total provider counts.
- **Governance events:** Validated `PorChallengePublicationV1` envelopes and
  `PorWeeklyReportV1` reports share the embedded node's durable outbox and
  runtime-signed canonical Governance DAG chain. Startup fails when that
  publisher is absent. The scheduler reports only the previous completed ISO
  week; its end boundary is the canonical `generated_at`, and prepared bytes
  plus the publication acknowledgement survive restart. Failed or
  committed-unknown publication is retried exactly, an unpublished cycle
  blocks advancement, and extended downtime catches up one week at a time.
  The report endpoint returns the retained exact report when that cycle is
  currently prepared. Status, export, and report endpoints expose coordinator
  history as canonical Norito payloads.
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

The local SF-9 state/report integration and verified drand/provider-VRF feeds
are implemented. Release promotion remains blocked on reviewed live deployment
evidence and any production governance archive handoff required by the
operator.

## Integration with Repair Automation
- For a `Failed` `AuditVerdictV1`, Torii derives the exactly-once source as
  `BLAKE3("sorafs.por.repair-source.v1" || challenge_id)` and derives the
  chain task identifier with `sorafs_repair_task_id_v1`.
- The canonical payload-free `RepairReportV1` uses
  `ticket_id = "POR-" + uppercase_hex(challenge_id)`, the runtime transaction
  authority, the verdict decision time, and a typed `por_failure` cause carrying
  only `challenge_id`, `failed_samples`, and the optional proof digest.
- Torii fails closed when the runtime repair signer, finalized cursor, or
  durable forwarder is unavailable. Exact source replay is idempotent; the same
  source with different canonical report bytes is rejected as an identity
  conflict.
- `Success` and `Repaired` verdicts never create a repair task. Failed status
  exposes the exact 32-byte chain task identifier rather than a process-local
  repair-history sequence.
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
coordinator state, report latency above threshold, missing PoR metrics
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
- `sorafs_manifest::por` challenge, proof, status, provider summary, slashing
  event, versioned challenge-publication envelope, and validated weekly-report
  payloads.
- `PorCoordinator`, `PorCoordinatorRuntime`, optional Norito snapshot
  persistence, durable node Governance DAG outbox publication, and fail-closed
  Torii startup wiring.
- Authenticated capacity PoR proof/verdict submission routes plus `/v1/sorafs/por/status`,
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
