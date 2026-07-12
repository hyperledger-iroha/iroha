---
lang: ar
direction: rtl
source: docs/source/sorafs_por_validator_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5a6dd02c37ec5f40bf2f9109a6314dd75a23351f3a08f4bc025a5379c714bec
source_last_modified: "2026-07-10T10:11:25+00:00"
translation_last_reviewed: 2026-06-25
---

# SoraFS PoR Validator CLI & Reporting

## Goals & Scope
- Provide auditors, SRE, and governance teams with deterministic tooling to inspect, validate, and export Proof-of-Replication (PoR) data produced by the coordinator (SF-9a).
- Deliver command-line workflows and API surfaces that track scheduler-issued challenge lifecycles and publish weekly health reports without relying on ad hoc scripts.
- Ensure all outputs rely on Norito payloads, respect audit retention policies, and integrate cleanly with the repair automation and governance DAG pipelines.

## Status
The local SF-9b status/reporting surfaces are partially implemented. Torii
exposes PoR status, export, report, and ingestion endpoints backed by
`PorCoordinator`; `sorafs_cli por status`, `por export`, and `por report`
consume those endpoints; and `sorafs-validate por` performs deterministic
challenge/proof pair validation for offline fixture and release checks.
Manual and externally supplied challenge ingress is intentionally absent from
the first-release API. Live challenges can originate only from the verified
coordinator scheduler. The public surface likewise contains no command, client
method, or HTTP endpoint for recording manual success/failure observations.

Remaining SF-9b work is live auditor rollout evidence, production archive
handoff, and any richer proof-bundle inspection commands required by operators.
The shared SF-9 rollout gate in
`scripts/check_sorafs_por_rollout_evidence.py` now covers the validator replay,
reporting/archive, observability, and governance-approval evidence needed before
operators treat the local PoR validator/reporting surface as released, including
the exact `archive_backend` value (`sql` or `parquet`) for deployment-specific
archive handoff, while
`scripts/run_sorafs_por_rollout_evidence.py` provides the reviewed collection
planner. The shared checker exports its required top-level payload fields as
`EVIDENCE_REQUIRED_FIELDS`, and the planner includes the checker-backed
`evidence_contract` map in `--dry-run` output so validator/reporting operators
can review the exact SF-9 artifact contract before promotion, and the runner
validates the schema-closed collection plan, required kinds, thresholds,
external evidence map, evidence contract, and command steps before dry-run
output or verifier execution. Observability artifacts also bind `metric_count`
to the unique canonical `metrics` inventory and reject duplicate metric labels
before promotion can report ready.

## Validator Personas
- **Auditor:** Independent or council-appointed reviewer validating provider proofs, filing slashing proposals, and verifying remediation.
- **SRE / Ops:** Monitors live scheduler challenges and exports digests for dashboards and incident review.
- **Governance Analyst:** Consumes weekly reports, reviews outstanding failures, and confirms that penalties/slashing align with policy.

## CLI Design (`sorafs_cli por ...`)

### Command Inventory
| Command | Description | Output |
|---------|-------------|--------|
| `sorafs_cli por status --torii-url=URL [--manifest=HEX32] [--provider=HEX32] [--epoch=N] [--status=pending|verified|failed|repaired|forced] [--limit=N] [--page-token=HEX32] [--format=table|json]` | List challenge statuses from Torii. | Table or JSON `Vec<PorChallengeStatusV1>`. |
| `sorafs_cli por export --torii-url=URL --out=PATH [--start-epoch=N] [--end-epoch=N]` | Download the coordinator status export. | Raw `PorStatusExportV1` bytes written to disk. |
| `sorafs_cli por report --torii-url=URL --week=YYYY-Www [--format=markdown|json]` | Render a weekly coordinator report. | Markdown or JSON `PorWeeklyReportV1`. |
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
    mean_latency_ms: Option<U64>,
    p95_latency_ms: Option<U64>,
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
    success_rate_bps: U16,
    first_failure_at: Option<Timestamp>,
    last_success_latency_ms_p95: Option<U32>,
    repair_dispatched: Bool,
    pending_repairs: U32,
    ticket_id: Option<String>,
}
```

The CLI serialises these types using Norito JSON (`norito::json`) and ensures
report outputs embed metadata (schema version, hash, generation timestamp).
All consensus-adjacent report metrics use integer units: success rate is
`0..=10_000` basis points and latency is an unsigned millisecond count. Provider
lists are sorted canonically, duplicate provider IDs are rejected, and offender
ties are resolved by provider ID so identical challenge histories produce
byte-identical Norito reports on every host.

## Torii API Extensions
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/sorafs/por/status` | Query `PorChallengeStatusV1` records filtered by manifest, provider, epoch, status, limit, and page token. |
| `GET` | `/v1/sorafs/por/export` | Return a Norito `PorStatusExportV1` for an optional epoch range. |
| `GET` | `/v1/sorafs/por/report/{iso_week}` | Return a Norito `PorWeeklyReportV1` generated from coordinator history. |
| `GET` | `/v1/sorafs/por/ingestion/{manifest_digest_hex}?limit=N` | Return `limit`-bounded provider backlog and last verdict timestamps from `sorafs_node`, with total provider counts retained. |
| `POST` | `/v1/sorafs/capacity/por-proof` | Record a provider `PorProofV1`; requires a fresh operator request signature whose Ed25519 key matches both the proof signer and the provider's current admitted advert key. |
| `POST` | `/v1/sorafs/capacity/por-verdict` | Record an auditor `AuditVerdictV1`; every unique signature must belong to the configured operator trust set, the authenticated request signer must be one of them, and `torii.sorafs_por.auditor_signature_threshold` must be met. |

The proof and verdict mutation routes use the canonical `x-iroha-operator-*` request-signature
envelope. Method, path, canonical query, exact body digest, timestamp, and nonce
are signed; stale timestamps, reused nonces, cross-path replays, body changes,
and keys outside the configured operator allow-list fail before payload
processing. PoR mutations fail closed when operator request signatures are
disabled, even if another operator-auth fallback is configured. Provider proof
and auditor verdict signatures use domain-separated canonical Norito payloads
(`sorafs.por.proof.signature.v1` and
`sorafs.por.verdict.signature.v1`). The proof signature covers the version,
challenge, manifest, provider, ordered samples, authentication path, and
`submitted_at`; the verdict signature covers the complete decision and
metadata except the signatures themselves.

Torii derives the trusted verdict-auditor set from the configured operator
signature allow-list plus the node key when `allow_node_key` is enabled, filters
it to Ed25519, and requires the non-zero
`torii.sorafs_por.auditor_signature_threshold`. The manifest, coordinator, and
node layers independently re-check that policy before committing state, so a
self-signed key embedded by an attacker is never a trust root.

The `/v1/sorafs/storage/por-challenge`, `por-proof`, and `por-verdict` method/path
pairs are not registered. Keeping one authenticated capacity lifecycle prevents
a direct-storage route from bypassing the coordinator, admission binding,
replay protection, or auditor checks.

`ManualPorChallengeV1` remains an offline fixture/tooling type only. Torii does
not admit manual or externally supplied challenges. The verified scheduler is
the only permitted production authority for the `PorChallengeV1` contract.
Production startup currently
rejects `torii.sorafs_por.enabled = true` because no authenticated external
drand/VRF feed is wired; deterministic seed material is explicitly not a
substitute.

## Offline Verification Pipeline
- Implemented: `sorafs-validate por` loads Norito `PorChallengeV1` and
  `PorProofV1`, validates payload shape, seed/deadline policy, challenge/proof
  binding, exact ordered sample-index coverage, and the provider's
  domain-separated proof signature, then emits `ValidationOutcomeV1`. Offline
  verification proves integrity but does not establish live provider admission.
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
- HTTP proof and verdict mutations are authenticated with fresh,
  replay-protected operator signatures. Every verdict signer must be
  allow-listed in `torii.operator_signatures`, and the configured auditor
  threshold must be met; provider keys must additionally match the current
  governance-admitted provider advert. External challenge ingestion and manual
  observation method/path pairs are unregistered; the scheduler is the trusted
  challenge authority.
- Proofs must cover the exact ordered sample indices in the challenge and carry
  a provider timestamp inside the inclusive issue/deadline window. Successful
  or repaired verdicts require the recorded proof digest; failure verdicts may
  omit it only when no proof arrived. Provider/manifest/digest/time mismatches
  leave the legitimate challenge retryable. Exact challenge, proof, and verdict
  replays are rejected, including attempts to resurrect a finalized challenge.
  The Torii coordinator/node pipeline is serialized and uses compensating
  rollback when the second state store rejects a transition. Coordinator
  snapshot failures and repair-history persistence failures roll back the
  in-memory transition before the other store is changed.
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
policy, governance archive handoff, and the exact `archive_backend` value (`sql`
or `parquet`). Raw challenge, proof, report,
export, response-body, token, transaction, and secret material is rejected.
The planner's dry-run output includes the same checker-owned field contract for
the selected required kinds.

## Rollout Status
Implemented locally:
- `PorChallengeStatusV1`, `PorWeeklyReportV1`, `PorProviderSummaryV1`,
  `PorSlashingEventV1`, `ManualPorChallengeV1`, and `PorStatusExportV1`.
- Torii status, export, report, ingestion, provider-proof, auditor-verdict, and
  authenticated provider-VRF routes.
- `sorafs_cli por status`, `por export`, and `por report`.
- `sorafs-validate por` challenge/proof pair validation.
- Focused tests for CLI status/export/report behavior, Torii
  status/export/report handlers, and removed-route absence.
- Shared fail-closed SF-9 rollout evidence gate, collection planner, operator
  argfile templates, and focused Python tests for validator/reporting evidence,
  including exact SQL/Parquet archive backend enforcement for reporting/archive
  evidence and negative route-registration checks for forbidden manual ingress.

Remaining production gates:
- Add proof-bundle fetch/show/offline replay commands if operators need them
  beyond `sorafs-validate por`.
- Archive live auditor, drand, VRF, report, and export evidence before treating
  SF-9 as fully released, and require that evidence to pass the SF-9 gate.
