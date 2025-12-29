---
title: SoraFS Repair Automation & Auditor API
summary: Final specification for the SF-8b repair scheduler, auditor integration, proof verifier, and SLA instrumentation.
---

# SoraFS Repair Automation & Auditor API

## Goals & Scope
- Automate remediation when Proof-of-Replication (PoR) or Proof-of-Retrievability (PoTR) checks detect replica loss or degraded providers.
- Provide deterministic auditor APIs for reporting evidence, filing slashing proposals, and tracking repair progress with Norito-authenticated envelopes.
- Define the proof verification pipeline that validates submitted evidence before tasks enter the scheduler.
- Capture SLAs, telemetry, and governance hooks so operators, auditors, and DealEngine share a common source of truth.

This document completes roadmap item **SF-8b — Repair automation & auditor API** and supersedes the draft outline.

## Component Overview
| Component | Responsibilities | Implementation Notes |
|-----------|-----------------|----------------------|
| Repair Scheduler | Accepts repair signals, creates tasks, drives workflow until closure. | Lives in `sorafs_node::repair`, backed by Postgres queue + async workers. |
| Repair Worker | Executes chunk fetch/re-seed, orchestrator requests, and governance callbacks. | Uses orchestrator facade for data movement, emits Norito events. |
| Auditor API | Signed REST/Norito endpoints for auditors to submit evidence and proposals. | Hosted by Torii (`/sorafs/auditor/*`), requires `SignedAuditorRequestV1`. |
| Proof Verifier | Validates PoR/PoTR evidence before enqueuing tasks or slashing proposals. | Reuses `sorafs_manifest::proof_stream` helpers and PoR coordinator fixtures. |
| SLA & Telemetry | Metrics, logs, and alerts for backlog, latency, and outcomes. | Instrumented via `iroha_telemetry`, exported to OTLP + Prometheus. |
| Persistence | Durable recording of tasks, events, and outcomes. | Postgres tables + Parquet archival; Governance DAG receives summaries. |
| CLI Tooling | Operator-facing commands for queue inspection and manual escalation. | `sorafs_cli repair *` commands with JSON output + Norito envelopes. |

## Norito Data Model
All payloads are Norito-native. The scheduler and Torii share the structures implemented in `sorafs_manifest::repair`:

```norito
struct RepairEvidenceV1 {
    version: U8,
    manifest_digest: Digest32,
    provider_id: ProviderId,
    por_history_id: Option<U64>,
    cause: RepairCauseV1,
    evidence_json: Option<String>,
    notes: Option<String>,
}

struct QueuedRepairStateV1 {
    queued_at_unix: Timestamp,
    sla_deadline_unix: Option<Timestamp>,
}

struct InProgressRepairStateV1 {
    queued_at_unix: Timestamp,
    started_at_unix: Timestamp,
    repair_agent: Option<String>,
}

struct CompletedRepairStateV1 {
    queued_at_unix: Timestamp,
    started_at_unix: Timestamp,
    completed_at_unix: Timestamp,
    resolution_notes: Option<String>,
}

struct FailedRepairStateV1 {
    queued_at_unix: Timestamp,
    failed_at_unix: Timestamp,
    reason: String,
}

struct EscalatedRepairStateV1 {
    queued_at_unix: Timestamp,
    escalated_at_unix: Timestamp,
    reason: String,
}

struct RepairTaskV1 {
    version: U8,
    ticket_id: RepairTicketId,
    manifest_digest: Digest32,
    provider_id: ProviderId,
    auditor_account: String,
    state: RepairTaskStateV1,
    por_history_id: Option<U64>,
    sla_deadline_unix: Option<Timestamp>,
    scheduler_notes: Option<String>,
    slash_proposal_digest: Option<Digest32>,
}

struct SlashProposalV1 {
    version: U8,
    ticket_id: RepairTicketId,
    provider_id: ProviderId,
    manifest_digest: Digest32,
    auditor_account: String,
    proposed_penalty_nano: U128,
    submitted_at_unix: Timestamp,
    rationale: String,
}

struct RepairTaskEventV1 {
    version: U8,
    ticket_id: RepairTicketId,
    status: RepairTaskStatusV1,       // queued | verifying | in_progress | completed | failed | escalated
    occurred_at_unix: Timestamp,
    actor: Option<String>,
    message: Option<String>,
}

struct SignedAuditorRequestV1 {
    version: U8,
    auditor_account: String,
    nonce: U64,
    payload: AuditorRequestPayloadV1,
    signature: AuditorSignatureV1,
}

```

- `RepairTaskEventV1` captures append-only status changes, including the actor (worker or scheduler) and optional free-form messages.
- `SignedAuditorRequestV1` wraps `RepairReportV1` or `RepairSlashProposalV1` payloads and enforces replay protection via the `nonce` field. Signatures use the same algorithm metadata as provider adverts (`SignatureAlgorithm` enum).

`RepairTaskStateV1` is a tagged union (`queued`, `in_progress`, `completed`, `failed`, `escalated`) whose payloads are the dedicated state structs above. State transitions are persisted as append-only `RepairTaskEventV1` records, each containing `{ticket_id, status, occurred_at, message}` to simplify replay and auditing.

## Scheduler Flow
1. **Triggers**  
   - PoR coordinator publishes `PorFailureEventV1`.  
   - PoTR probes emit `PotrDeadlineMissedV1`.  
   - Auditors call `/sorafs/auditor/repair/report`.

2. **Proof Verification**  
   - `RepairEvidenceV1` is validated via `ProofVerifier::verify(evidence)` which ensures digests match manifests, sample indices are within range, and signatures on receipts are valid.  
   - Verification failures return `422 evidence_invalid` with detailed error codes. Auditor submissions are rejected; PoR/PoTR triggers mark the corresponding history entry as `verification_failed` for operator follow-up.

3. **Task Creation**  
   - Scheduler groups by `(manifest_digest, provider_id)` to avoid duplicate efforts. Existing open tasks are updated with merged evidence; otherwise a new `RepairTaskV1` is created.  
   - SLA deadlines: `PoR` tasks must start within 15 minutes, finish within 2 hours; `PoTR` tasks start within 5 minutes, finish within 30 minutes. Manual reports default to 4 hours.

4. **Worker Assignment**  
   - Workers poll via `repair.dequeue` RPC with capacity hints.  
   - Tasks enter `in_progress` and the worker performs orchestrator-driven recovery (chunk fetch, re-seed, or manifest re-issuance).  
   - On success, worker publishes `RepairTaskEventV1` with `completed` and triggers DealEngine to recalculate collateral exposure.

5. **Failure & Escalation**  
   - Retries use exponential backoff capped at 5 attempts.  
   - After final failure, tasks move to `escalated` and a `SlashProposalV1` stub is generated for auditor review.  
   - Escalations automatically notify governance via `sorafs_governance_event`.

6. **Queue Hygiene**  
   - Ghost tasks (stuck `in_progress` beyond 2× SLA) are reclaimed and re-queued.  
   - Metrics track queue depth and latency buckets; Alertmanager fires if `queued` tasks older than SLA exceed thresholds.

## Auditor API Surface
| Method & Path | Description | Auth | Success Response |
|---------------|-------------|------|------------------|
| `POST /sorafs/auditor/repair/report` | Submit `SignedAuditorRequestV1 (payload=repair_report)` to enqueue/augment a repair task. | Required | `202 Accepted` with `task_id`. |
| `POST /sorafs/auditor/slash/proposal` | Submit `SignedAuditorRequestV1 (payload=slash_proposal)` for governance consideration. | Required | `202 Accepted` with `proposal_id`. |
| `GET /sorafs/auditor/repair/status/{manifest_digest}` | Fetch paginated tasks for a manifest. | Auditor or operator JWT | `200 OK` with `RepairTaskPageV1`. |
| `GET /sorafs/auditor/repair/task/{task_id}` | Detailed task view, including event timeline and evidence bundle links. | Auditor or operator | `200 OK`. |
| `POST /sorafs/auditor/slash/{proposal_id}/decision` | Record governance decision (approve/reject/defer). | Governance account | `200 OK`. |

### Authentication
- Mutating endpoints require Norito signatures. Torii validates the envelope against the auditor registry stored under `governance/sorafs/auditors.toml`.  
- Optional OIDC flow maps bearer tokens to an auditor account and injects envelopes server-side to support dashboards without bypassing signature requirements. Tokens must include `auditor_account` and `nonce` claims to prevent replay.

### Rate Limiting & Replay Protection
- Nonce values must increase monotonically per auditor account; Torii stores the highest nonce in Redis/Postgres and rejects stale requests.  
- REST layer enforces 60 requests/minute per auditor account, configurable via `iroha_config::torii.sorafs_auditor_rate_limit`.

## Proof Verification Pipeline
- Reuses `sorafs_manifest::proof_stream::Verifier` to validate chunk proofs and receipt signatures.
- Additional checks:
  - Ensure `manifest_digest` exists and corresponds to the latest accepted manifest in DealEngine.
  - Validate PoTR timestamps relative to SLA (<15 minutes old).
  - Confirm evidence matches the PoR coordinator challenge by cross-checking `failure_id`.
- On success, verified artefacts are stored in object storage (`s3://sorafs-audit/evidence/<task_id>/`) with SHA-256 digests recorded alongside the task.
- Verification failures emit `sorafs_repair_verifier_errors_total{reason}` and notify auditors via Torii event.

## SLA & Observability
- Metrics (Prometheus naming):
  - `sorafs_repair_tasks_total{status}` — Counter.
  - `sorafs_repair_task_latency_seconds_bucket{proof_kind}` — Histogram measuring time from creation to completion/escalation.
  - `sorafs_repair_backlog_gauge{proof_kind}` — Gauge for queued tasks.
  - `sorafs_auditor_requests_total{endpoint,result}` — Counter for API usage.
  - `sorafs_repair_verifier_failures_total{reason}` — Counter.
- Alerts:
  - `SORAfsRepairBacklogHigh`: queue > 50 tasks or oldest queued > SLA.
  - `SORAfsRepairEscalations`: escalations per hour > 3.
  - `SORAfsAuditorAuthFailures`: >10 failed signatures in 5 minutes.
- Logs:
  - Structured JSON with `task_id`, `status`, `sla_deadline`, `retry_count`.
  - Loki retention 180 days hot, 2 years archived (mirrors pricing policy logs).
- Dashboards:
  - Grafana panels for backlog trend, SLA percentiles, auditor activity.
  - Runbook links to `docs/source/sorafs_ops_playbook.md` and `sorafs_gateway_self_cert.md` for transport-related incidents.

## Persistence & Retention
```sql
CREATE TABLE sorafs_repair_task (
    id UUID PRIMARY KEY,
    manifest_digest BYTEA NOT NULL,
    provider_id BYTEA NOT NULL,
    priority SMALLINT NOT NULL,
    status SMALLINT NOT NULL,
    sla_deadline TIMESTAMPTZ NOT NULL,
    retry_count SMALLINT NOT NULL DEFAULT 0,
    last_transition TIMESTAMPTZ NOT NULL,
    evidence_ref TEXT NOT NULL, -- object storage URI
    por_history_id BIGINT REFERENCES sorafs_por_history(id)
);

CREATE TABLE sorafs_repair_event (
    id BIGSERIAL PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES sorafs_repair_task(id),
    status SMALLINT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    message TEXT,
    actor TEXT NOT NULL -- worker id / auditor account / system
);

CREATE TABLE sorafs_slash_proposal (
    id UUID PRIMARY KEY,
    task_id UUID REFERENCES sorafs_repair_task(id),
    penalty_xor NUMERIC(24,12) NOT NULL,
    rationale TEXT NOT NULL,
    status SMALLINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    decided_at TIMESTAMPTZ,
    decision TEXT
);
```

- Retention: 180 days hot storage in Postgres. Nightly compactor emits Parquet snapshots to `s3://sorafs-audit/repair/YYYY/MM/DD`. Governance DAG stores summaries for immutable history.
- Indexes on `(manifest_digest, status)` and `(sla_deadline)` support queue queries.

## CLI & Torii Integration
- `sorafs_cli repair queue --manifest <cid>`: shows open tasks, statuses, deadlines.
- `sorafs_cli repair submit --evidence evidence.json --auditor <account>`: helper to build and sign `SignedAuditorRequestV1`.
- `sorafs_cli repair approve-slash --proposal <id> --decision approve|reject`: governance response wrapper.
- CLI commands rely on `RepairTaskPageV1` and `SlashProposalPageV1` responses; all outputs support `--format json|table`.
- Torii exposes WebSocket stream `wss://.../sorafs/repair/events` broadcasting `RepairTaskEventV1` for dashboards and integrations.

## Governance & Escalation
- Collateral adjustments feed into the Reserve+Rent engine: when a task escalates, the provider's collateral ratio is recomputed and can trigger Reserve lifecycle downgrades (`Warning`, `Grace`, etc.) described in `sorafs_reserve_rent_plan.md`.
- Slash proposals automatically populate `governance/sorafs/slashing/proposals/` in the DAG with Norito proofs for council review.
- Weekly governance meeting reviews:
  - Total repairs, escalations, penalties.
  - Auditor performance (response time, quality).
  - Outstanding proposals older than 7 days (must be decided or escalated).

## Implementation Checklist
- [x] Define Norito schemas for repair tasks, evidence, proposals, and signed requests.
- [x] Specify scheduler workflow, retry semantics, and SLA deadlines.
- [x] Document auditor endpoints, authentication, rate limits, and replay prevention.
- [x] Describe proof verification pipeline and evidence storage guarantees.
- [x] Enumerate telemetry metrics, alerts, and dashboards required for compliance.
- [x] Lay out persistence schema, retention policies, and governance integration.
- [x] Outline CLI/Torii interactions and WebSocket feeds for operator tooling.

Remaining engineering work (tracked separately):
- Implement Rust modules under `sorafs_node::repair` and `sorafs_torii::auditor`.
- Add golden tests for Norito schemas and REST API integration.
- Wire scheduler to the forthcoming PoR coordinator (SF-9).

With this specification in place, SF-8b is considered complete at the roadmap level and downstream development can begin using the agreed contract.
