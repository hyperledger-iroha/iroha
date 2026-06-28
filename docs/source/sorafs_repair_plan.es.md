---
lang: es
direction: ltr
source: docs/source/sorafs_repair_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dc4d510176f12b96075467cdef53e54e186c67affa3128eee429f4eef5200196
source_last_modified: "2026-06-25T17:31:52+00:00"
translation_last_reviewed: 2026-06-25
---

# SoraFS Repair Automation & Auditor API

## Goals & Scope
- Automate remediation when Proof-of-Replication (PoR) or Proof-of-Retrievability (PoTR) checks detect replica loss or degraded providers.
- Provide deterministic auditor APIs for reporting evidence, filing slashing proposals, and tracking repair progress with Norito-authenticated envelopes.
- Track the validation pipeline that checks submitted evidence before tasks enter the scheduler.
- Capture SLAs, telemetry, and governance hooks so operators, auditors, and DealEngine share a common source of truth.

## Status
The local SF-8b repair foundations are implemented. `sorafs_node` owns the
repair scheduler, state snapshot, PoR history linkage, worker leases, local and
orchestrator-backed rehydration paths, watchdog escalation, governance
audit/slash publication hooks, and GC protection for active repair tasks. Torii
exposes signed auditor report/slash endpoints, repair worker claim/heartbeat/
complete/fail endpoints, status listings, JSON and Norito
`SignedAuditorRequestV1` decoding, per-auditor nonce replay protection, worker
permission checks, and local sequenced repair event JSON/SSE/WebSocket streams.
`iroha sorafs repair` and `iroha sorafs gc` expose the
operator CLI surfaces, and `sorafs-validate repair` validates repair payload
fixtures through the SF-11 reference outcome contract.

Remaining SF-8b work is live operator evidence: archive a production PoR/PoTR
failure, repair, escalation, and governance handoff once the deployed auditor
roster and SF-9 coordinator publish their runbooks.
`scripts/check_sorafs_repair_rollout_evidence.py` now provides the fail-closed
SF-8b rollout evidence gate for deployed repair promotion packets, and
`scripts/run_sorafs_repair_rollout_evidence.py` provides the matching reviewed
evidence collection planner/runner. Signed auditor API, worker lifecycle,
event stream, governance handoff, and approval artifacts must bind to a valid
auditor roster digest; worker, event stream, and handoff artifacts must also
bind to a valid failure-capture evidence bundle digest. Roster or failure
bundle mismatches are recorded on the offending artifact in the JSON summary
before required-kind validity is reported.

## Component Overview
| Component | Responsibilities | Implementation Notes |
|-----------|-----------------|----------------------|
| Repair Scheduler | Accepts repair signals, creates tasks, drives workflow until closure. | Lives in `sorafs_node::repair`, backed by an on-disk Norito snapshot store + async workers. |
| Repair Worker | Executes local rehydration, chunk fetch/re-seed, orchestrator requests, and governance callbacks. | Attempts local rehydration from co-located manifests before invoking the optional repair orchestrator hook for remote fetches. |
| Auditor API | Signed REST/Norito endpoints for auditors to submit evidence and proposals. | Hosted by Torii under `/v1/sorafs/audit/repair/*`, requires `SignedAuditorRequestV1` for report/slash submissions. |
| Payload Validator | Validates repair records, evidence, reports, slash proposals, escalation policy/approval payloads, worker signatures, audit events, and signed auditor envelopes. | Implemented in `sorafs_manifest::repair` plus `sorafs_manifest::reference::validate_repair_payload_bytes`; live PoR/PoTR replay evidence remains rollout work. |
| SLA & Telemetry | Metrics, logs, and alerts for backlog, latency, and outcomes. | Instrumented via `iroha_telemetry`, exported to OTLP + Prometheus. |
| Persistence | Durable recording of tasks, events, and outcomes. | Norito snapshot (`repair_state.to`) in `sorafs.repair.state_dir`; Governance DAG receives summaries. |
| CLI Tooling | Operator-facing commands for queue inspection, manual escalation, and GC inspection/dry-run. | `iroha sorafs repair *` and `iroha sorafs gc *` commands with JSON output + Norito envelopes. |

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
- `SignedAuditorRequestV1` wraps `RepairReportV1` or `RepairSlashProposalV1` payloads. Torii accepts JSON or Norito `SignedAuditorRequestV1` bodies on the `/report` and `/slash` endpoints, validates the envelope version, non-zero nonce, auditor-account match, payload kind, Ed25519 signature over the canonical signed payload, signer key binding to the canonical auditor account, enforces persistent per-auditor monotonic nonce replay checks, and rejects legacy raw `RepairReportV1`/`RepairSlashProposalV1` request bodies. Signatures use the same algorithm metadata as provider adverts (`SignatureAlgorithm` enum).
- `RepairAuditEventV1` and `GcAuditEventV1` wrap payloads with deterministic ordering metadata plus signer/digest fields for governance audit trails.

`RepairTaskStateV1` is a tagged union (`queued`, `in_progress`, `completed`, `failed`, `escalated`) whose payloads are the dedicated state structs above. State transitions are persisted as append-only `RepairTaskEventV1` records, each containing `{ticket_id, manifest_digest, provider_id, status, occurred_at, message}` to simplify replay and auditing.

## Scheduler Flow
1. **Triggers**
   - PoR coordinator publishes `PorFailureEventV1`.
   - PoTR probes emit `PotrDeadlineMissedV1`.
   - Auditors call `/v1/sorafs/audit/repair/report`.

2. **Payload Validation**
   - `RepairEvidenceV1`, reports, slash proposals, escalation approvals, worker signatures, audit events, and signed auditor envelopes are validated through `sorafs_manifest::repair` and `sorafs_manifest::reference::validate_repair_payload_bytes`.
   - Validation failures reject auditor submissions before scheduler mutation
     and return structured Torii errors for operator follow-up.

3. **Task Creation**
   - Scheduler groups by `(manifest_digest, provider_id)` to avoid duplicate efforts. Existing open tasks are updated with merged evidence; otherwise a new `RepairTaskV1` is created.
   - SLA deadlines: `PoR` tasks must start within 15 minutes, finish within 2 hours; `PoTR` tasks start within 5 minutes, finish within 30 minutes. Manual reports default to 4 hours.

4. **Worker Assignment**
   - Workers claim tickets via `/v1/sorafs/audit/repair/claim` and must send `/v1/sorafs/audit/repair/heartbeat` updates before the lease TTL elapses.
   - Tasks enter `in_progress` and the worker performs orchestrator-driven recovery (chunk fetch, re-seed, or manifest re-issuance).
   - Workers close tickets with `/v1/sorafs/audit/repair/complete` or `/v1/sorafs/audit/repair/fail`, and Torii rejects stale/out-of-order updates based on lease TTL + heartbeat cadence.

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
| `POST /v1/sorafs/audit/repair/report` | Submit a signed `SignedAuditorRequestV1` carrying `RepairReportV1` to enqueue a repair task. | Required | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `POST /v1/sorafs/audit/repair/slash` | Submit a signed `SignedAuditorRequestV1` carrying `RepairSlashProposalV1` for governance consideration. | Required | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `GET /v1/sorafs/audit/repair/status` | List repair tasks across manifests (filter by provider/status). | Auditor or operator JWT | `200 OK` with records (Norito base64). |
| `GET /v1/sorafs/audit/repair/status/{manifest_hex}` | Fetch repair tasks for a manifest. | Auditor or operator JWT | `200 OK` with records (Norito base64). |
| `GET /v1/sorafs/audit/repair/events` | Poll the local sequenced repair event backlog (`since`, `limit`) with ETag support. | Torii rate limit/perimeter | `200 OK` JSON containing `events[]` and `next_since`. |
| `GET /v1/sorafs/audit/repair/events/stream` | Replay the selected backlog, then stream live repair task transitions as Server-Sent Events. | Torii rate limit/perimeter | `text/event-stream` frames keyed by repair status. |
| `GET /v1/sorafs/audit/repair/events/ws` | Replay the selected backlog, then stream live repair task transitions over WebSocket JSON frames. | Torii rate limit/perimeter | WebSocket frames `{event, data}` keyed by repair status. |

## Worker API Surface
| Method & Path | Description | Auth | Success Response |
|---------------|-------------|------|------------------|
| `POST /v1/sorafs/audit/repair/claim` | Claim a queued repair ticket (`manifest_digest_hex`, `worker_id`, `claimed_at_unix`, `idempotency_key`). | Worker signature + `CanOperateSorafsRepair` | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `POST /v1/sorafs/audit/repair/heartbeat` | Renew the active lease (`manifest_digest_hex`, `heartbeat_at_unix`, `idempotency_key`). | Worker signature + `CanOperateSorafsRepair` | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `POST /v1/sorafs/audit/repair/complete` | Complete a ticket (`manifest_digest_hex`, `completed_at_unix`, optional notes, `idempotency_key`). | Worker signature + `CanOperateSorafsRepair` | `200 OK` with `RepairTaskRecordV1` (Norito base64). |
| `POST /v1/sorafs/audit/repair/fail` | Fail a ticket (`manifest_digest_hex`, `failed_at_unix`, reason, `idempotency_key`). | Worker signature + `CanOperateSorafsRepair` | `200 OK` with `RepairTaskRecordV1` (Norito base64). |

### Repair Lease & Idempotency
- Claims expire after `sorafs.repair.claim_ttl_secs`; heartbeats extend leases by `sorafs.repair.heartbeat_interval_secs`.
- Torii rejects stale/out-of-order timestamps, lease mismatches, retry claims before backoff expiry, and idempotency key reuse with different payloads.

### Authentication
- Auditor endpoints (`/report`, `/slash`) accept JSON or Norito `SignedAuditorRequestV1` envelopes and reject invalid envelope versions, zero nonces, auditor-account mismatches, wrong payload kind, unsupported signature algorithms, invalid Ed25519 signatures, signer keys that do not match the canonical auditor account, and stale or replayed per-auditor nonces. Legacy raw `RepairReportV1`/`RepairSlashProposalV1` request bodies are rejected.
- Repair worker endpoints (`/claim`, `/heartbeat`, `/complete`, `/fail`) require a `RepairWorkerSignaturePayloadV1` signature from the worker account (i105 account id/signatory key) plus the on-chain `CanOperateSorafsRepair` permission for the ticket's provider. The signed payload includes `manifest_digest`, `provider_id`, action summary, and timestamps for auditability; `manifest_digest_hex` must match the ticket record. Provider owners receive this permission automatically and may delegate/revoke it via `GrantPermission`. No admin-only repair overrides are supported in production paths.
- Dashboard or service integrations must submit the same signed envelope or
  worker-signature payload as CLI/API clients; no token-to-envelope injection
  path is shipped in the current Torii implementation.

### Rate Limiting & Replay Protection
- Signed auditor envelopes require a non-zero `nonce`; Torii persists the highest accepted nonce per canonical auditor account in the repair state snapshot and rejects any signed report or slash proposal whose nonce is less than or equal to the stored value.
- Torii applies the existing origin/perimeter limiter before decoding and a
  dedicated per-auditor limiter after signed-envelope validation and before
  scheduler mutation. Configure it with
  `sorafs.repair.auditor_rate_per_sec` and
  `sorafs.repair.auditor_burst`; defaults are 4 requests/second with a burst of
  16, and setting either value to `0` disables that side of the limiter.

## Validation Pipeline
- Implemented local validation covers canonical Norito decoding, schema
  versions, non-zero digests and provider identifiers, timestamps, SLA policy,
  escalation approval policy, signed auditor envelope signatures, worker action
  signatures, payload kind matching, and auditor nonce replay protection.
- `sorafs-validate repair` exposes the same checks for fixture and release
  validation across repair evidence, reports, task records, slash proposals,
  escalation policy/approval payloads, signed auditor requests, worker
  signatures, task events, and audit events.
- Production PoR/PoTR Merkle replay archives, object-storage evidence retention,
  and auditor notification wiring remain live rollout evidence items.

The rollout evidence scripts have focused Python coverage in:

- `scripts/tests/check_sorafs_repair_rollout_evidence_test.py`
- `scripts/tests/run_sorafs_repair_rollout_evidence_test.py`

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
- Snapshot schema captures `version`, `next_por_history_id`, `next_audit_sequence`, `tasks[]` (report, state, lease, governance votes/decisions, events), `por_history[]`, and `auditor_nonces[]` with the highest accepted nonce per canonical auditor account.
- Writes are atomic (temp file + rename) to avoid partial state on restart; corrupted snapshots are archived with a `corrupt-*` suffix before reinitialisation.
- Retention is enforced by capping `RepairTaskEventV1` history per ticket; governance audit events remain append-only in the DAG for immutable history.

## CLI & Torii Integration
- `iroha sorafs repair list --manifest-digest <hex>`: shows tasks, statuses, and deadlines.
- `iroha sorafs repair claim --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a worker claim.
- `iroha sorafs repair complete --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a completion update.
- `iroha sorafs repair fail --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a failure update.
- `iroha sorafs repair escalate --ticket-id <id> --manifest-digest <hex> --provider-id <hex> --penalty-nano <n> --rationale <text>`: submits a slash proposal for governance review (approval summary optional).
- `iroha sorafs repair escalate ... --approve-votes <n> --approved-at <ts> --finalized-at <ts> [--reject-votes <n>] [--abstain-votes <n>]`: attaches a governance approval summary when a decision is already recorded.
- `iroha sorafs gc inspect`: reports retained manifests and retention deadlines (read-only).
- `iroha sorafs gc dry-run`: reports only expired manifests that GC would evict (read-only).
- CLI commands return JSON payloads from Torii (Norito-encoded values rendered as JSON).
- Repair status listings include ordered `RepairTaskEventV1` logs for auditability, capped to the most recent transitions to bound payload size.
- Torii exposes dedicated local repair event backlog, SSE, and WebSocket
  endpoints at `/v1/sorafs/audit/repair/events`,
  `/v1/sorafs/audit/repair/events/stream`, and
  `/v1/sorafs/audit/repair/events/ws`. The stream wrapper adds a local
  monotonic `sequence` around the canonical `RepairTaskEventV1` payload without
  changing the canonical repair task event, governance audit event, or Norito
  repair state schema.
- The generated Torii OpenAPI document advertises all three repair event routes,
  including `since` and `limit` query parameters for backlog replay.

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

## Rollout Evidence Gate

Use the rollout gate after the deployed auditor roster, SF-9 coordinator,
PoR/PoTR failure capture, signed auditor API, repair worker lifecycle, repair
event streams, governance handoff, observability, and governance packet have
produced reviewed, payload-free JSON evidence:

```sh
python3 scripts/check_sorafs_repair_rollout_evidence.py \
  @scripts/examples/sorafs_repair_rollout_evidence.args.example
```

For staged collections with reviewed evidence paths, prefer the planner so the
verifier command and summary path are reproducible:

```sh
python3 scripts/run_sorafs_repair_rollout_evidence.py \
  @scripts/examples/sorafs_repair_rollout_collection.args.example \
  --dry-run
```

The checker recognizes `sorafs.repair.*` SF-8b rollout schemas for auditor
roster, failure capture, signed auditor API, worker lifecycle, event streams,
governance handoff, observability, and governance approval evidence. It reports
`ready` only when every required kind is present, every recognized artifact is
valid, raw PoR/PoTR evidence, raw repair payloads, signed auditor requests,
response bodies, signed transactions, secrets, and ledgers are absent, route
latency, event lag, and repair latency stay under configured thresholds, the
auditor roster meets the configured minimum, governance is bound to
`iroha_config`, auditor API / worker lifecycle / event stream / governance
handoff / governance approval artifacts carry a `roster_digest_hex` that
matches a valid auditor-roster artifact, and worker lifecycle / event stream /
governance handoff artifacts carry an `evidence_bundle_digest_hex` that matches
a valid PoR/PoTR failure-capture artifact in the same rollout bundle.

## Rollout Status
Implemented engineering coverage:
- `sorafs_node::repair` owns the scheduler, persistent repair store, PoR history linkage, worker leases, watchdog escalation, and governance audit/slash publication hooks.
- Torii exposes the auditor repair/slash submission paths with signed `SignedAuditorRequestV1` envelopes, nonce replay protection, and Norito/JSON request validation.
- Local golden and integration coverage now exercises the repair schemas, scheduler state transitions, auditor request validation, PoR failure binding, REST endpoints, and repair worker flows.
- `iroha sorafs repair` and `iroha sorafs gc` provide operator CLI coverage;
  `sorafs-validate repair` provides release/fixture validation.
- The SF-8b rollout evidence gate, collection planner, operator argfile
  templates, and focused tests are implemented for payload-free deployed
  evidence review, including cross-artifact auditor-roster and failure-bundle
  digest binding.

Remaining rollout work is live operator evidence: collect production PoR
failure, repair, and governance handoff artifacts once the deployed auditor
roster and SF-9 coordinator publish their runbooks, then pass the SF-8b rollout
evidence gate.
