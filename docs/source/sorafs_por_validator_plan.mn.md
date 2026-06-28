---
lang: mn
direction: ltr
source: docs/source/sorafs_por_validator_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59681c3f49093f091d40a455fa21f24faf442a2ce9af0d63dcde3bac6b6a9d3b
source_last_modified: "2026-06-25T16:58:37+00:00"
translation_last_reviewed: 2026-06-25
title: SoraFS PoR Validator CLI & Reporting
summary: SF-9b implementation status for PoR validator tooling, status/report/export APIs, CLI commands, and remaining audit rollout evidence.
---

# SoraFS PoR Validator CLI & Reporting

## Goals & Scope
- Provide auditors, SRE, and governance teams with deterministic tooling to inspect, validate, and export Proof-of-Replication (PoR) data produced by the coordinator (SF-9a).
- Deliver command-line workflows and API surfaces that track challenge lifecycles, trigger manual probes, and publish weekly health reports without relying on ad hoc scripts.
- Ensure all outputs rely on Norito payloads, respect audit retention policies, and integrate cleanly with the repair automation and governance DAG pipelines.

## Status
The local SF-9b status/reporting surfaces are partially implemented. Torii
exposes PoR status, export, report, and ingestion endpoints backed by
`PorCoordinator`; `sorafs_cli por status`, `por export`, and `por report`
consume those endpoints; and `sorafs-validate por` performs deterministic
challenge/proof pair validation for offline fixture and release checks.
`sorafs_cli por trigger` can still construct and submit the legacy manual
trigger request shape, but Torii deliberately retires
`POST /v1/sorafs/por/trigger` with a fail-closed `410 Gone` response because
manual challenge admission must be governed by the scheduler/capacity challenge
path before any live challenge is committed.

Remaining SF-9b work is live auditor rollout evidence, production archive
handoff, and any richer proof-bundle inspection commands required by operators.
The shared SF-9 rollout gate in
`scripts/check_sorafs_por_rollout_evidence.py` now covers the validator replay,
reporting/archive, observability, and governance-approval evidence needed before
operators treat the local PoR validator/reporting surface as released, while
`scripts/run_sorafs_por_rollout_evidence.py` provides the reviewed collection
planner.

## Validator Personas
- **Auditor:** Independent or council-appointed reviewer validating provider proofs, filing slashing proposals, and verifying remediation.
- **SRE / Ops:** Monitors live challenges, triggers manual probes during incidents, and exports digests for dashboards.
- **Governance Analyst:** Consumes weekly reports, reviews outstanding failures, and confirms that penalties/slashing align with policy.

## CLI Design (`sorafs_cli por ...`)

### Command Inventory
| Command | Description | Output |
|---------|-------------|--------|
| `sorafs_cli por status --torii-url=URL [--manifest=HEX32] [--provider=HEX32] [--epoch=N] [--status=pending|verified|failed|repaired|forced] [--limit=N] [--page-token=HEX32] [--format=table|json]` | List challenge statuses from Torii. | Table or JSON `Vec<PorChallengeStatusV1>`. |
| `sorafs_cli por export --torii-url=URL --out=PATH [--start-epoch=N] [--end-epoch=N]` | Download the coordinator status export. | Raw `PorStatusExportV1` bytes written to disk. |
| `sorafs_cli por report --torii-url=URL --week=YYYY-Www [--format=markdown|json]` | Render a weekly coordinator report. | Markdown or JSON `PorWeeklyReportV1`. |
| `sorafs_cli por trigger --torii-url=URL --manifest=HEX32 --provider=HEX32 --reason=TEXT --auth-token=PATH [--samples=N] [--deadline-secs=N]` | Construct the legacy manual challenge request with a Norito auth token. | Posts to the trigger endpoint and surfaces the Torii retirement response; live challenges must use governed `PorChallengeV1` submission. |
| `sorafs-validate por --challenge <challenge.to> --proof <proof.to> --format json` | Validate a committed or downloaded challenge/proof pair offline. | `ValidationOutcomeV1`. |

The shipped `sorafs_cli por` commands use `--torii-url=URL` key/value syntax.
The offline validator remains in `sorafs-validate` so it can share the SF-11
reference outcome contract.

### Norito Payloads
Canonical structs live in `sorafs_manifest::por` alongside SF-9a schemas:

```norito
struct PorChallengeStatusV1 {
    version: U8,
    challenge_id: Digest32,
    manifest_digest: Digest32,
    provider_id: Digest32,
    epoch_id: U64,
    drand_round: U64,
    status: PorChallengeOutcome,      // pending|verified|failed|repaired|forced
    sample_count: U16,
    forced: Bool,
    issued_at: Timestamp,
    responded_at: Option<Timestamp>,
    proof_digest: Option<Digest32>,
    repair_task_id: Option<Bytes16>,
    failure_reason: Option<String>,
    verifier_latency_ms: Option<U32>,
}

struct PorWeeklyReportV1 {
    version: U8,
    cycle: PorReportIsoWeek,
    generated_at: Timestamp,
    challenges_total: U32,
    challenges_verified: U32,
    challenges_failed: U32,
    forced_challenges: U32,
    repairs_enqueued: U32,
    repairs_completed: U32,
    mean_latency_ms: Option<F64>,
    p95_latency_ms: Option<F64>,
    slashing_events: Vec<PorSlashingEventV1>,
    providers_missing_vrf: Vec<Digest32>,
    top_offenders: Vec<PorProviderSummaryV1>,
    notes: Option<String>,
}

struct PorSlashingEventV1 {
    provider_id: Digest32,
    manifest_digest: Digest32,
    penalty_xor: XorAmount,
    verdict_cid: String,
    decided_at: Timestamp,
}

struct PorProviderSummaryV1 {
    provider_id: Digest32,
    manifest_count: U32,
    challenges: U32,
    successes: U32,
    failures: U32,
    forced: U32,
    success_rate: F64,
    first_failure_at: Option<Timestamp>,
    last_success_latency_ms_p95: Option<U32>,
    repair_dispatched: Bool,
    pending_repairs: U32,
    ticket_id: Option<String>,
}
```

The CLI serialises these types using Norito JSON (`norito::json`) and ensures report outputs embed metadata (schema version, hash, generation timestamp).

## Torii API Extensions
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/sorafs/por/status` | Query `PorChallengeStatusV1` records filtered by manifest, provider, epoch, status, limit, and page token. |
| `GET` | `/v1/sorafs/por/export` | Return a Norito `PorStatusExportV1` for an optional epoch range. |
| `GET` | `/v1/sorafs/por/report/{iso_week}` | Return a Norito `PorWeeklyReportV1` generated from coordinator history. |
| `GET` | `/v1/sorafs/por/ingestion/{manifest_digest_hex}?limit=N` | Return `limit`-bounded provider backlog and last verdict timestamps from `sorafs_node`, with total provider counts retained. |
| `POST` | `/v1/sorafs/por/trigger` | Return a fail-closed `410 Gone` retirement response for the legacy manual trigger route. |
| `POST` | `/v1/sorafs/capacity/por-challenge` | Record a governance-issued `PorChallengeV1`. |
| `POST` | `/v1/sorafs/capacity/por-proof` | Record a provider `PorProofV1`. |
| `POST` | `/v1/sorafs/capacity/por-verdict` | Record an auditor `AuditVerdictV1` and update coordinator status. |

`ManualPorChallengeV1` includes `{manifest_digest, provider_id,
requested_samples, requested_deadline_secs, reason}`. The CLI request builder is
present so existing operator scripts fail with a structured server response
instead of a missing route. Torii does not admit manual triggers through this
route; governed challenge payloads must be submitted through
`/v1/sorafs/capacity/por-challenge` or a scheduler runtime that records the same
`PorChallengeV1` contract.

## Offline Verification Pipeline
- Implemented: `sorafs-validate por` loads Norito `PorChallengeV1` and
  `PorProofV1`, validates payload shape, seed/deadline policy, challenge/proof
  binding, and exact sample-index coverage, then emits `ValidationOutcomeV1`.
- Implemented: `sorafs_cli por status/export/report` consumes coordinator
  history for audit review.
- Not yet shipped: single-challenge display, proof-bundle download, and richer
  offline replay commands that fetch deployment-specific Merkle archives.

## Reporting & Dashboards
- Weekly reports are generated on demand from `PorCoordinator::weekly_report`
  and returned by Torii as Norito `PorWeeklyReportV1` payloads.
- Markdown output mirrors governance meeting structure:
  ```
  # PoR Weekly Report (2026-W08)
  - Total challenges: 1344 (verified 1310, failed 22, forced 12)
  - Repairs enqueued: 18 (completed 15, outstanding 3)
  - Slashing events: 1 (provider sorafs:prov:abc, penalty 120 XOR, verdict CID ipfs://...)
  - Providers missing VRF: sorafs:prov:def (3 epochs), sorafs:prov:xyz (1 epoch)
  - Notes: ...
  ```
- Dashboards and alert fixtures cover scheduler failures, forced challenges,
  duplicate samples, ingestion backlog, and ingestion failures. Production
  weekly-report ingestion remains deployment work.
- Alerting:
  - `SORAfsPorWeeklyFailures` triggers if `challenges_failed > 0.05 * challenges_total`.
  - `SORAfsPorWeeklyVRFMiss` triggers if any provider misses VRF > 3 epochs.

## Governance & Audit Workflow
- Reports and manual challenge requests should be authorized by
  governance-approved material. The current manual-trigger CLI validates a
  Norito auth token before submitting, and the live trigger route returns an
  explicit retirement response until the governed scheduler path can commit the
  resulting `PorChallengeV1`.
- Export files currently contain the raw Norito `PorStatusExportV1` payload.
  Parquet/manifest packaging and SoraFS pinning are production archive tasks.
- Governance meetings can reference `PorWeeklyReportV1` to decide on penalties,
  certify reparations, and update public transparency logs once live evidence is
  archived.

## Rollout Evidence Gate

The SF-9 validator/reporting release claim is tied to the same fail-closed gate
used by the scheduler plan:

```bash
python3 scripts/check_sorafs_por_rollout_evidence.py \
  @scripts/examples/sorafs_por_rollout_evidence.args.example
```

For reviewed collection planning:

```bash
python3 scripts/run_sorafs_por_rollout_evidence.py \
  @scripts/examples/sorafs_por_rollout_collection.args.example \
  --dry-run
```

The validator-specific evidence must prove `sorafs-validate por` challenge/proof
replay, challenge/proof binding, exact sample coverage, deadline policy,
Merkle/archive replay, `ValidationOutcomeV1` schema compatibility, bounded
status/export/report route latency, weekly report generation, archive-retention
policy, governance archive handoff, and the explicit `retired` decision for the
manual-trigger server route. Raw challenge, proof, report,
export, response-body, token, transaction, and secret material is rejected.

## Rollout Status
Implemented locally:
- `PorChallengeStatusV1`, `PorWeeklyReportV1`, `PorProviderSummaryV1`,
  `PorSlashingEventV1`, `ManualPorChallengeV1`, and `PorStatusExportV1`.
- Torii status, export, report, ingestion, and capacity PoR submission routes.
- Torii `POST /v1/sorafs/por/trigger` retirement route returning `410 Gone` with
  `route_state = "retired"` for the legacy manual-trigger surface.
- `sorafs_cli por status`, `por export`, `por report`, and manual trigger
  request construction.
- `sorafs-validate por` challenge/proof pair validation.
- Focused tests for CLI status/export/report/trigger behavior and Torii
  status/export/report handlers.
- Shared fail-closed SF-9 rollout evidence gate, collection planner, operator
  argfile templates, and focused Python tests for validator/reporting evidence.

Remaining production gates:
- Include the manual-trigger route retirement decision in the SF-9 gate evidence.
- Add proof-bundle fetch/show/offline replay commands if operators need them
  beyond `sorafs-validate por`.
- Archive live auditor, drand, VRF, report, and export evidence before treating
  SF-9 as fully released, and require that evidence to pass the SF-9 gate.
