---
lang: ur
direction: rtl
source: docs/source/sorafs_repair_plan.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: fc8591f4083fd1bf08af5a0785c86bf3c6111f4401540fb2d9b77cc8a84fcfb1
source_last_modified: "2026-07-06T19:45:48.800622+00:00"
translation_last_reviewed: 2026-07-03
source_mtime: "2026-07-06T19:45:48.800622+00:00"
---

# SoraFS Repair Automation & Auditor API

## Goals & Scope
- Automate remediation when Proof-of-Replication (PoR) or Proof-of-Retrievability (PoTR) checks detect replica loss or degraded providers.
- Provide deterministic auditor APIs for reporting evidence, filing slashing proposals, and tracking repair progress with Norito-authenticated envelopes.
- Track the validation pipeline that checks submitted evidence before tasks enter the scheduler.
- Capture SLAs, telemetry, and governance hooks so operators, auditors, and DealEngine share a common source of truth.

## Status
The SF-8b operational foundations and native ledger model are implemented.
The ledger owns canonical task identity, source binding, compare-and-set
leases, terminal outcomes, slash state, appeals, typed events, and singular
queries. `sorafs_node` still owns a separate `FileRepairStore`, worker
scheduler, event sequence, PoR history linkage, local/orchestrator rehydration,
watchdog escalation, governance publication hooks, and GC protection. Torii's
signed auditor and worker routes still mutate and read that local store.
Those paths are useful test/development projections, but they are not a
production authority and cannot satisfy multi-peer exactly-once repair.

Remaining SF-8b implementation work is to submit every auditor/worker
transition as a native transaction, reconcile finalized task and event
queries, rebuild daemon state solely from committed history, and delete the
authoritative local mutation path. After that cutover, operators must archive a
production PoR/PoTR failure, repair, escalation, and governance handoff from
the reviewed four-validator deployment.
`scripts/check_sorafs_repair_rollout_evidence.py` now provides the fail-closed
SF-8b rollout evidence gate for deployed repair promotion packets, and
`scripts/run_sorafs_repair_rollout_evidence.py` provides the matching reviewed
evidence collection planner/runner. The checker exports its required top-level
payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the planner includes the
checker-backed `evidence_contract` map in dry-run output for the selected
required kinds, and validates the schema-closed collection plan, required
kinds, thresholds, external evidence map, evidence contract, and command steps
before dry-run output or verifier execution. The shared runner plan guard also
rejects non-canonical nested required-kind, threshold, external-evidence,
evidence-contract, and command-step shapes before dry-run output or verifier
execution. Signed auditor API, worker lifecycle, event stream, governance
handoff, and approval artifacts must bind to a valid
auditor roster digest; worker, event stream, and handoff artifacts must also
bind to a valid failure-capture evidence bundle digest; governance approval
artifacts must additionally bind to the valid governance handoff digest. Roster,
failure-bundle, or handoff-digest mismatches are recorded on the offending
artifact in the JSON summary before required-kind validity is reported.
Governance handoff artifacts also publish `policy_digest_hex`; governance
approval artifacts must bind `policy_digest_hex` to that valid handoff policy
digest, and the checker emits those valid handoff policies as
`valid_policy_digests`. Signed auditor API, worker lifecycle, and event stream
artifacts also bind `route_count` to the unique canonical `routes[].name`
inventory and reject duplicate or unknown route entries before promotion can
report ready. Every signed repair route response must include a
`body_blake3_hex` digest before auditor, worker, or event readiness can report
ready.
Auditor-roster artifacts also bind `auditor_count` to the unique canonical
`auditors[].name` inventory, require reviewed `repair-auditor-*` labels without
non-production markers, and reject duplicate auditor entries before promotion
can report ready.
Failure-capture artifacts also bind `failure_source_count` to the unique
canonical `failure_sources` inventory and `failure_event_count` to the unique
canonical `failure_events[].name` inventory, require reviewed
`repair-failure-event-*` labels without non-production markers, require
reviewed failure events to cover both PoR and PoTR sources, and reject
duplicate or unknown source entries plus duplicate event entries.
Observability artifacts also bind `metric_count` to the unique canonical
`metrics` inventory, require the reviewed repair metrics, and reject duplicate
or unknown metric labels before promotion can report ready.
Repair payload-safety artifacts must explicitly set `raw_roster_included`,
`raw_evidence_included`, `response_bodies_included`,
`raw_repair_payloads_included`, `raw_ledger_included`, and
`critical_alerts_firing` to `false` before promotion can report ready.
`scripts/build_sorafs_repair_canary.py` builds individual payload-free SF-8b
canary artifacts for auditor roster, failure capture, signed auditor API,
worker lifecycle, event streams, governance handoff, observability, and
governance approval evidence. The builder requires reviewed deployment
context, complete failure-source, failure-event, repair route,
lifecycle-status, handoff-target, and metric coverage where applicable,
rejects duplicate or unknown failure-source, route, lifecycle-status,
handoff-target, and metric inputs before writing,
auditor-roster and failure-bundle digest bindings, governance handoff digest
bindings, auditor minimum counts, reviewed `repair-auditor-*` labels and
`repair-failure-event-*` failure events matching their counts, default
`--failure-event-count` values derived from the reviewed failure-source
inventory, derived
`status_count` and `handoff_target_count`
fields for reviewed lifecycle-status and handoff-target inventories, route,
event-lag, and repair-latency threshold facts,
config-backed governance metadata, reviewed policy-digest input for
governance handoff and approval evidence, and
validates every generated artifact through
`scripts/check_sorafs_repair_rollout_evidence.py` before writing. Checked-in
response-file examples cover auditor-roster and worker-lifecycle canaries.

## Component Overview
| Component | Responsibilities | Implementation Notes |
|-----------|-----------------|----------------------|
| Repair Scheduler | Accepts repair signals, creates tasks, drives workflow until closure. | Native repair state is authoritative; the current `sorafs_node::repair` file store and async workers must be converted into a finalized-chain projection and transaction submitter. |
| Repair Worker | Executes local rehydration, chunk fetch/re-seed, orchestrator requests, and governance callbacks. | Attempts local rehydration from co-located manifests before invoking the optional repair orchestrator hook for remote fetches. |
| Auditor API | Signed REST/Norito endpoints for auditors to submit evidence and proposals. | Hosted by Torii under `/v1/sorafs/audit/repair/*`, requires `SignedAuditorRequestV1` for report/slash submissions. |
| Payload Validator | Validates repair records, evidence, reports, slash proposals, escalation policy/approval payloads, worker signatures, audit events, and signed auditor envelopes. | Implemented in `sorafs_manifest::repair` plus `sorafs_manifest::reference::validate_repair_payload_bytes`; live PoR/PoTR replay evidence remains rollout work. |
| SLA & Telemetry | Metrics, logs, and alerts for backlog, latency, and outcomes. | Instrumented via `iroha_telemetry`, exported to OTLP + Prometheus. |
| Persistence | Durable recording of tasks, events, and outcomes. | Native ledger records and finalized event queries are the V1 authority. `repair_state.to` is a rebuildable development cache only and must not gate production transitions. |
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
    proposed_penalty: U128,
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
- `RepairAuditEventV1` and `GcAuditEventV1` wrap payloads with deterministic ordering metadata plus signer/digest fields for governance audit trails. Envelope validation is mandatory before publication: sequence numbers are non-zero, header and payload timestamps are identical, repair signers equal the payload actor (or the canonical `sorafs-repair` fallback), and GC signers equal `sorafs-gc`. `payload_digest` is BLAKE3 over the canonical header-bearing `norito::to_bytes(payload)` archive; bare codec payloads are never accepted as the digest preimage.
- GC audit payloads use the closed first-release reason vocabulary `retention_expired` or `retention_expired_provider_missing`; only the latter permits an all-zero provider identifier. Blocked outcomes use exactly `repair_active`, `deal_active`, or `shared_chunks` and must report zero freed bytes. Successful evictions may also report zero for valid empty manifests. Unknown labels or inconsistent provider/outcome fields fail before any governance artifact is written.

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
   - After the final failure (or SLA breach), tasks move to `escalated` and a `RepairSlashProposalV1` draft is generated using `sorafs.repair.default_slash_penalty` for auditor review.
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
| `max_penalty` | 1,000,000,000 | Maximum slash penalty allowed for repair escalations (nano-XOR). |

- Scheduler-generated proposals are capped at `max_penalty`; auditor submissions above the cap are rejected.
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
- The native ledger is the production persistence boundary for repair task,
  lease, terminal, slash, appeal, and event state.
- The current daemon projection is persisted as a Norito snapshot
  (`repair_state.to`) under `sorafs.repair.state_dir` (defaults to
  `<sorafs.storage.data_dir>/repair` when unset). It is not authoritative and
  must become fully rebuildable from finalized native queries.
- Snapshot schema captures `version`, `next_por_history_id`, `next_audit_sequence`, `tasks[]` (report, state, lease, governance votes/decisions, events), `por_history[]`, and `auditor_nonces[]` with the highest accepted nonce per canonical auditor account.
- Writes use private, no-follow, bounded atomic replacement plus file and parent-directory sync. Startup rejects non-canonical, corrupt, truncated, trailing, unsafe-path, hard-linked, over-limit, unordered, duplicate, or forged snapshots; production recovery must discard/rebuild this projection rather than let it override committed repair state.
- Snapshot decoding enforces independent per-sequence and per-field ceilings plus cumulative element, allocation, and nesting-depth budgets before semantic validation. Embedded slash-proposal archives have a tighter schema-derived 4 KiB field/sequence ceiling and their canonical bytes, digest, and publication stage must remain an inseparable tuple.
- Cross-process locking and a persisted checkpoint digest reject stale local writers. Until cutover, sequence/nonce reservations, idempotency caches, dropped-event counts, PoR history, and governance votes share the snapshot; production V1 must instead derive transition ordering and idempotency from committed transactions and finalized event cursors.
- Retention is enforced by capping `RepairTaskEventV1` history per ticket; governance audit events remain append-only in the DAG for immutable history.

## CLI & Torii Integration
- `iroha sorafs repair list --manifest-digest <hex>`: shows tasks, statuses, and deadlines.
- `iroha sorafs repair claim --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a worker claim.
- `iroha sorafs repair complete --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a completion update.
- `iroha sorafs repair fail --ticket-id <id> --manifest-digest <hex> --provider-id <hex>`: signs a failure update.
- `iroha sorafs repair escalate --ticket-id <id> --manifest-digest <hex> --provider-id <hex> --penalty <quantity> --rationale <text>`: submits an unapproved slash proposal for governance review. The CLI never accepts or embeds vote counts or approval timestamps; after cutover, decisions must derive exclusively from authenticated votes committed to the native repair ledger.
- `iroha sorafs gc inspect`: reports retained manifests and retention deadlines (read-only).
- `iroha sorafs gc dry-run`: reports only expired manifests that GC would evict (read-only).
- CLI commands return JSON payloads from Torii (Norito-encoded values rendered as JSON).
- Repair status listings include ordered `RepairTaskEventV1` logs for auditability, capped to the most recent transitions to bound payload size. Every snapshot also returns `events_dropped`, the exact number of omitted oldest events, so truncation is never silent.
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
| `max_penalty` | `1,000,000,000` | Cap on slash penalties (nano-XOR). |

`RepairEscalationApprovalV1` remains a canonical output/reference payload for a
decision derived from stored votes; it is not accepted as proposal authority.
Slash proposals carrying an embedded approval summary fail closed. Unapproved
proposals remain in dispute until authenticated votes satisfy the minimum-voter,
quorum, dispute-window, and appeal-window policy, and penalties are capped to
the policy maximum.

## Rollout Evidence Gate

Use the rollout gate only after the native finalized-chain cutover and after
the deployed auditor roster, SF-9 coordinator, PoR/PoTR failure capture,
signed auditor API, repair worker lifecycle, committed repair event streams,
governance handoff, observability, and governance packet have produced
reviewed, payload-free JSON evidence:

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
timing fields are non-negative integer-unit evidence, the auditor roster meets
the configured minimum, governance is bound to `iroha_config`, auditor API /
worker lifecycle / event stream / governance
handoff / governance approval artifacts carry a `roster_digest_hex` that
matches a valid auditor-roster artifact, and worker lifecycle / event stream /
governance handoff artifacts carry an `evidence_bundle_digest_hex` that matches
a valid PoR/PoTR failure-capture artifact in the same rollout bundle, and
governance approval artifacts carry a `handoff_digest_hex` that matches a valid
governance handoff artifact.
Governance handoff artifacts also carry `policy_digest_hex`; the checker emits
those values as `valid_policy_digests`, and governance approval artifacts must
carry a matching `policy_digest_hex`. Signed auditor API, worker lifecycle, and
event stream artifacts must also keep `route_count` equal to the unique
canonical `routes[].name` inventory and reject duplicate or unknown route
entries. Every route response must carry a `body_blake3_hex` digest.
Auditor-roster artifacts also bind `auditor_count` to the unique canonical
`auditors[].name` inventory, require reviewed `repair-auditor-*` labels without
non-production markers, and reject duplicate auditor entries before promotion
can report ready.
Failure-capture artifacts must also keep `failure_source_count` equal to the
unique canonical `failure_sources` inventory, keep `failure_event_count` equal
to the unique canonical `failure_events[].name` inventory, require reviewed
`repair-failure-event-*` labels without non-production markers, require
reviewed failure events to cover both PoR and PoTR sources, and reject
duplicate or unknown source entries plus duplicate event entries. Worker lifecycle artifacts
require `status_count`, bind it to the unique canonical `statuses_observed`
inventory, and reject missing, inflated, duplicate, or unknown lifecycle-status
evidence. Governance handoff artifacts require `handoff_target_count`, bind it
to the unique canonical `handoff_targets` inventory, and reject missing,
inflated, duplicate, or unknown handoff-target evidence. Observability artifacts
also bind `metric_count` to the unique canonical `metrics` inventory, require
the reviewed repair metrics, and reject duplicate or unknown metric labels
before promotion can report ready.
The summary exports the sorted reviewed `metrics` inventory plus
`metric_count_values`, and the aggregate production-readiness gate requires
those fields to match the observability artifact fingerprint before final
promotion can report ready. The repair gate fail-closes when more than one
valid roster, failure bundle, handoff, or policy anchor appears, and clears the
mixed `valid_roster_digests`, `valid_failure_bundle_digests`,
`valid_handoff_digests`, or `valid_policy_digests` set before aggregate
promotion can report ready.
Aggregate promotion also rechecks the lane-proven repair digest relationships:
roster-bound artifact fingerprints must match `valid_roster_digests`,
failure-bound artifact fingerprints must match `valid_failure_bundle_digests`,
handoff-bound artifact fingerprints must match `valid_handoff_digests`, and
policy-bound artifact fingerprints must match `valid_policy_digests` before
final promotion can report ready.
Its collection planner exposes those exact required payload fields through
`--dry-run` and validates the schema-closed collection plan, required kinds,
thresholds, external evidence map, evidence contract, and command steps before
touching live repair services. The shared runner plan guard rejects
non-canonical nested required-kind, threshold, external-evidence,
evidence-contract, and command-step shapes before any live repair contact.

## Rollout Status
Implemented engineering coverage:
- `sorafs_node::repair` owns the scheduler, persistent repair store, PoR history linkage, worker leases, watchdog escalation, and governance audit/slash publication hooks.
- Torii exposes the auditor repair/slash submission paths with signed `SignedAuditorRequestV1` envelopes, nonce replay protection, and Norito/JSON request validation.
- Local golden and integration coverage now exercises the repair schemas, scheduler state transitions, auditor request validation, PoR failure binding, REST endpoints, and repair worker flows.
- `iroha sorafs repair` and `iroha sorafs gc` provide operator CLI coverage;
  `sorafs-validate repair` provides release/fixture validation.
- `scripts/build_sorafs_repair_canary.py` provides checked-in payload-free
  canary generation for the local SF-8b rollout gate.
- The SF-8b rollout evidence gate, collection planner, operator argfile
  templates, and focused tests are implemented for payload-free deployed
  evidence review, including cross-artifact auditor-roster and failure-bundle
  digest binding plus governance handoff digest and handoff policy digest
  binding.

Remaining implementation work is the finalized-chain cutover: replace
`FileRepairStore` mutations and local event streams with native repair
transactions and committed task/event projections, then prove cross-peer
exactly-once execution and restart reconciliation. Remaining rollout work after
that cutover is genuine operator evidence for a production PoR/PoTR failure,
repair, escalation, and governance handoff, followed by the SF-8b rollout
evidence gate.
