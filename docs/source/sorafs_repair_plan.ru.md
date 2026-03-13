---
lang: ru
direction: ltr
source: docs/source/sorafs_repair_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4ae98523f111b9e02ef19e0996926c2927c9c76dc752ef33e82ebe8c917c3ea
source_last_modified: "2026-01-30T09:08:14.786000+00:00"
translation_last_reviewed: 2026-01-30
---

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
| Repair Scheduler | Accepts repair signals, creates tasks, drives workflow until closure. | Lives in `sorafs_node::repair`, backed by an on-disk Norito snapshot store + async workers. |
| Repair Worker | Executes local rehydration, chunk fetch/re-seed, orchestrator requests, and governance callbacks. | Attempts local rehydration from co-located manifests before invoking the optional repair orchestrator hook for remote fetches. |
| Auditor API | Signed REST/Norito endpoints for auditors to submit evidence and proposals. | Hosted by Torii (`/sorafs/auditor/*`), requires `SignedAuditorRequestV1`. |
| Proof Verifier | Validates PoR/PoTR evidence before enqueuing tasks or slashing proposals. | Reuses `sorafs_manifest::proof_stream` helpers and PoR coordinator fixtures. |
| SLA & Telemetry | Metrics, logs, and alerts for backlog, latency, and outcomes. | Instrumented via `iroha_telemetry`, exported to OTLP + Prometheus. |
| Persistence | Durable recording of tasks, events, and outcomes. | Norito snapshot (`repair_state.to`) in `sorafs.repair.state_dir`; Governance DAG receives summaries. |
| CLI Tooling | Operator-facing commands for queue inspection, manual escalation, and GC inspection/dry-run. | `iroha app sorafs repair *` and `iroha app sorafs gc *` commands with JSON output + Norito envelopes. |

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
    manifest_digest: Digest32,
    provider_id: ProviderId,
    status: RepairTaskStatusV1,       // queued | verifying | in_progress | completed | failed | escalated
    occurred_at_unix: Timestamp,
    actor: Option<String>,
    message: Option<String>,
}

struct SorafsAuditHeaderV1 {
    sequence: U64,                    // monotonic for deterministic ordering
    occurred_at_unix: Timestamp,
    signer: String,
    payload_digest: Digest32,
}

struct RepairAuditEventV1 {
    version: U8,
    header: SorafsAuditHeaderV1,
    payload: RepairTaskEventV1,
}

struct GcAuditPayloadV1 {
    version: U8,
    manifest_digest: Digest32,
    provider_id: ProviderId,
    evicted_at_unix: Timestamp,
    freed_bytes: U64,
    reason: String,
}

struct GcAuditEventV1 {
    version: U8,
    header: SorafsAuditHeaderV1,
    payload: GcAuditPayloadV1,
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
- `SignedAuditorRequestV1` wraps `RepairReportV1` or `RepairSlashProposalV1` payloads and enforces replay protection via the `nonce` field (planned; Torii currently accepts raw `RepairReportV1`/`RepairSlashProposalV1`). Signatures use the same algorithm metadata as provider adverts (`SignatureAlgorithm` enum).
- `RepairAuditEventV1` and `GcAuditEventV1` wrap payloads with deterministic ordering metadata plus signer/digest fields for governance audit trails.

`RepairTaskStateV1` is a tagged union (`queued`, `in_progress`, `completed`, `failed`, `escalated`) whose payloads are the dedicated state structs above. State transitions are persisted as append-only `RepairTaskEventV1` records, each containing `{ticket_id, manifest_digest, provider_id, status, occurred_at, message}` to simplify replay and auditing.

## Scheduler Flow
1. **Triggers**  
   - PoR coordinator publishes `PorFailureEventV1`.  
   - PoTR probes emit `PotrDeadlineMissedV1`.  
   - Auditors call `/v2/sorafs/audit/repair/report`.

2. **Proof Verification**  
   - `RepairEvidenceV1` is validated via `ProofVerifier::verify(evidence)` which ensures digests match manifests, sample indices are within range, and signatures on receipts are valid.  
   - Verification failures return `422 evidence_invalid` with detailed error codes. Auditor submissions are rejected; PoR/PoTR triggers mark the corresponding history entry as `verification_failed` for operator follow-up.

3. **Task Creation**  
   - Scheduler groups by `(manifest_digest, provider_id)` to avoid duplicate efforts. Existing open tasks are updated with merged evidence; otherwise a new `RepairTaskV1` is created.  
   - SLA deadlines: `PoR` tasks must start within 15 minutes, finish within 2 hours; `PoTR` tasks start within 5 minutes, finish within 30 minutes. Manual reports default to 4 hours.

4. **Worker Assignment**  
   - Workers claim tickets via `/v2/sorafs/audit/repair/claim` and must send `/v2/sorafs/audit/repair/heartbeat` updates before the lease TTL elapses.  
   - Tasks enter `in_progress` and the worker performs orchestrator-driven recovery (chunk fetch, re-seed, or manifest re-issuance).  
   - Workers close tickets with `/v2/sorafs/audit/repair/complete` or `/v2/sorafs/audit/repair/fail`, and Torii rejects stale/out-of-order updates based on lease TTL + heartbeat cadence.

5. **Failure & Escalation**  
   - Retries use exponential backoff derived from `sorafs.repair.backoff_initial_secs` and `sorafs.repair.backoff_max_secs`, with `sorafs.repair.max_attempts` enforcing the cap.  
   - After the final failure (or SLA breach), tasks move to `escalated` and a `RepairSlashProposalV1` draft is generated using `sorafs.repair.default_slash_penalty_nano` for auditor review.  
   - Escalations automatically notify governance via `sorafs_governance_event`.

6. **Governance Decision**  
   - Escalations open a dispute window (`governance.sorafs_repair_escalation.dispute_window_secs`) during which governance voters submit approve/reject votes.  
   - At the dispute deadline (`escalated_at_unix + dispute_window_secs`) the decision is computed deterministically: require `minimum_voters`, approvals exceed rejections, and the approval ratio (basis points) meets `quorum_bps`. Ties or insufficient quorum reject the slash.  
   - Approved decisions open an appeal window (`appeal_window_secs`); appeals recorded within the window mark the decision as appealed and halt automatic slashing.

7. **Queue Hygiene**  
   - The watchdog reclaims expired leases, re-queues the ticket with backoff, and escalates if the attempt cap is exceeded.  
   - Metrics track queue depth and latency buckets; Alertmanager fires if `queued` tasks older than SLA exceed thresholds.

## Governance Escalation Policy
The escalation policy is sourced from `governance.sorafs_repair_escalation` in `iroha_config` and is enforced for every repair slash proposal.

| Setting | Default | Meaning |
|---------|---------|---------|
| `quorum_bps` | 6667 | Minimum approval ratio (basis points) among counted votes. |
| `minimum_voters` | 3 | Minimum number of distinct voters required to resolve a decision. |
| `dispute_window_secs` | 86400 | Time after escalation before votes are finalized (seconds). |
| `appeal_window_secs` | 604800 | Time after approval during which appeals are accepted (seconds). |
| `max_penalty_nano` | 1,000,000,000 | Maximum slash penalty allowed for repair escalations (nano-XOR). |

- Scheduler-generated proposals are capped at `max_penalty_nano`; auditor submissions above the cap are rejected.
- Vote records are stored in `repair_state.to` with deterministic ordering (`voter_id` sorting) so all nodes derive the same decision timestamp and outcome.

## Auditor API Surface
| Method & Path | Description | Auth | Success Response |
|---------------|-------------|------|------------------|
| `POST /v2/sorafs/audit/repair/report` | Submit `RepairReportV1` to enqueue a repair task. | Required | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `POST /v2/sorafs/audit/repair/slash` | Submit `RepairSlashProposalV1` for governance consideration. | Required | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `GET /v2/sorafs/audit/repair/status` | List repair tasks across manifests (filter by provider/status). | Auditor or operator JWT | `200 OK` with records (Norito base64). |
| `GET /v2/sorafs/audit/repair/status/{manifest_hex}` | Fetch repair tasks for a manifest. | Auditor or operator JWT | `200 OK` with records (Norito base64). |

## Worker API Surface
| Method & Path | Description | Auth | Success Response |
|---------------|-------------|------|------------------|
| `POST /v2/sorafs/audit/repair/claim` | Claim a queued repair ticket (`manifest_digest_hex`, `worker_id`, `claimed_at_unix`, `idempotency_key`). | Worker signature + `CanOperateSorafsRepair` | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `POST /v2/sorafs/audit/repair/heartbeat` | Renew the active lease (`manifest_digest_hex`, `heartbeat_at_unix`, `idempotency_key`). | Worker signature + `CanOperateSorafsRepair` | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `POST /v2/sorafs/audit/repair/complete` | Complete a ticket (`manifest_digest_hex`, `completed_at_unix`, optional notes, `idempotency_key`). | Worker signature + `CanOperateSorafsRepair` | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `POST /v2/sorafs/audit/repair/fail` | Fail a ticket (`manifest_digest_hex`, `failed_at_unix`, reason, `idempotency_key`). | Worker signature + `CanOperateSorafsRepair` | `200 OK` with `RepairTaskRecordV1` (Norito base64). |

### Repair Lease & Idempotency
- Claims expire after `sorafs.repair.claim_ttl_secs`; heartbeats extend leases by `sorafs.repair.heartbeat_interval_secs`.
- Torii rejects stale/out-of-order timestamps, lease mismatches, retry claims before backoff expiry, and idempotency key reuse with different payloads.

### Authentication
- Auditor endpoints (`/report`, `/slash`) require Norito signatures; Torii validates envelopes against the auditor registry stored under `governance/sorafs/auditors.toml` (raw payloads are accepted until `SignedAuditorRequestV1` is wired).
- Repair worker endpoints (`/claim`, `/heartbeat`, `/complete`, `/fail`) require a `RepairWorkerSignaturePayloadV1` signature from the worker account (I105 account id/signatory key) plus the on-chain `CanOperateSorafsRepair` permission for the ticket's provider. The signed payload includes `manifest_digest`, `provider_id`, action summary, and timestamps for auditability; `manifest_digest_hex` must match the ticket record. Provider owners receive this permission automatically and may delegate/revoke it via `GrantPermission`. No admin-only repair overrides are supported in production paths.
- Optional OIDC flow maps bearer tokens to an auditor account and injects envelopes server-side to support dashboards without bypassing signature requirements. Tokens must include `auditor_account` and `nonce` claims to prevent replay.

### Rate Limiting & Replay Protection
- Nonce values must increase monotonically per auditor account; Torii stores the highest nonce in the repair state snapshot and rejects stale requests.  
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
  - `torii_sorafs_repair_tasks_total{status}` — Counter for task transitions.
  - `torii_sorafs_repair_latency_minutes_bucket{outcome}` — Histogram measuring time from creation to completion/escalation.
  - `torii_sorafs_repair_queue_depth{provider}` — Gauge for queued tasks per provider.
  - `torii_sorafs_repair_backlog_oldest_age_seconds` — Age of the oldest queued task.
  - `torii_sorafs_repair_lease_expired_total{outcome}` — Counter for expired leases (requeued/escalated).
  - `torii_sorafs_slash_proposals_total{outcome}` — Counter for slash proposal transitions.
- Governance audit JSON metadata mirrors the telemetry labels (`ticket_id`, `manifest`, `provider`, `status` for repair events; `outcome` for slash proposals) to keep correlation deterministic.
- Alerts:
  - `SoraFsRepairBacklogHigh`: queue > 50 tasks or oldest queued > SLA.
  - `SoraFsRepairEscalations`: escalations per hour > 3.
  - `SoraFsRepairLeaseExpirySpike`: lease expiries per hour > 5.
- Logs:
  - Structured JSON with `task_id`, `status`, `sla_deadline`, `retry_count`.
  - Loki retention 180 days hot, 2 years archived (mirrors pricing policy logs).
- Dashboards:
  - Grafana panels for backlog trend, SLA percentiles, auditor activity.
  - Runbook links to `docs/source/sorafs_ops_playbook.md` and `sorafs_gateway_self_cert.md` for transport-related incidents.

## Persistence & Retention
- Repair state is persisted as a Norito snapshot (`repair_state.to`) under `sorafs.repair.state_dir` (defaults to `<sorafs.storage.data_dir>/repair` when unset).
- Snapshot schema captures `version`, `next_por_history_id`, `next_audit_sequence`, `tasks[]` (report, state, lease, governance votes/decisions, events), and `por_history[]`.
- Writes are atomic (temp file + rename) to avoid partial state on restart; corrupted snapshots are archived with a `corrupt-*` suffix before reinitialisation.
- Retention is enforced by capping `RepairTaskEventV1` history per ticket; governance audit events remain append-only in the DAG for immutable history.

## CLI & Torii Integration
- `iroha app sorafs repair list --manifest-digest <hex>`: shows tasks, statuses, and deadlines.
- `iroha app sorafs repair claim --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a worker claim.
- `iroha app sorafs repair complete --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a completion update.
- `iroha app sorafs repair fail --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a failure update.
- `iroha app sorafs repair escalate --ticket-id <id> --manifest-digest <hex> --provider-id <hex> --penalty-nano <n> --rationale <text>`: submits a slash proposal for governance review (approval summary optional).
- `iroha app sorafs repair escalate ... --approve-votes <n> --approved-at <ts> --finalized-at <ts> [--reject-votes <n>] [--abstain-votes <n>]`: attaches a governance approval summary when a decision is already recorded.
- `iroha app sorafs gc inspect`: reports retained manifests and retention deadlines (read-only).
- `iroha app sorafs gc dry-run`: reports only expired manifests that GC would evict (read-only).
- CLI commands return JSON payloads from Torii (Norito-encoded values rendered as JSON).
- Repair status listings include ordered `RepairTaskEventV1` logs for auditability, capped to the most recent transitions to bound payload size.
- Torii exposes WebSocket stream `wss://.../sorafs/repair/events` broadcasting `RepairTaskEventV1` for dashboards and integrations.

## Governance & Escalation
- Collateral adjustments feed into the Reserve+Rent engine: when a task escalates, the provider's collateral ratio is recomputed and can trigger Reserve lifecycle downgrades (`Warning`, `Grace`, etc.) described in `sorafs_reserve_rent_plan.md`.
- Slash proposals automatically populate `governance/sorafs/slashing/proposals/` in the DAG with Norito proofs for council review.
- Weekly governance meeting reviews:
  - Total repairs, escalations, penalties.
  - Auditor performance (response time, quality).
  - Outstanding proposals older than 7 days (must be decided or escalated).

### Escalation policy (defaults)

| Parameter | Default | Purpose |
| --- | --- | --- |
| `quorum_bps` | `6,667` (2/3) | Minimum approval ratio over approve/reject votes; ties are rejected. |
| `minimum_voters` | `3` | Minimum distinct votes (approve + reject + abstain) required to consider a decision. |
| `dispute_window_secs` | `86,400` (24h) | Minimum delay from escalation to approval. |
| `appeal_window_secs` | `604,800` (7d) | Minimum delay after approval before a decision is final. |
| `max_penalty_nano` | `1,000,000,000` | Cap on slash penalties (nano-XOR). |

Approval summaries may be attached to slash proposals via `RepairEscalationApprovalV1`:
`approve_votes`, `reject_votes`, `abstain_votes`, `approved_at_unix`, and
`finalized_at_unix`. When a summary is present, Torii rejects proposals that do
not meet quorum, minimum-voter, dispute-window, or appeal-window requirements,
and penalties are capped to the policy maximum. Proposals without a summary are
accepted and remain in dispute until votes resolve at the dispute deadline.

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
