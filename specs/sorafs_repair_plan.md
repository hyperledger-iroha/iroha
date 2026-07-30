---
title: SoraFS Repair Automation & Auditor API
summary: SF-8b implementation status for native repair authority, signed-transaction ingress, rebuildable worker projections, and remaining rollout evidence.
---

# SoraFS Repair Automation & Auditor API

## Goals & Scope
- Automate remediation when Proof-of-Replication (PoR) or Proof-of-Retrievability (PoTR) checks detect replica loss or degraded providers.
- Provide deterministic auditor and worker APIs for submitting caller-signed
  native transactions and tracking finalized repair state.
- Track the validation pipeline that checks submitted evidence before tasks enter the scheduler.
- Capture SLAs, telemetry, and governance hooks so operators, auditors, and the
  native orderbook/reserve projections share one finalized source of truth.

## Status
The SF-8b native ledger model and Torii command cutover are implemented. The
ledger owns canonical task identity, source binding, compare-and-set leases,
terminal outcomes, slash proposals, appeals, typed events, and singular
queries. Every command route accepts exactly one caller-signed
`SignedTransaction` containing the route-specific native repair instruction
and forwards it through strict durable transaction ingress. Reads return
finalized ledger projections; obsolete local status-by-manifest, SSE, and
WebSocket authority routes are not shipped.

The former local `RepairManager`, `FileRepairStore`, repair checkpoint,
mutation/event history, scheduler, and compatibility APIs have been deleted.
The storage executor accepts only a fully validated native task read at an
exact finalized cursor and requires the current lease owner, generation,
revision, provider binding, and expiry before any storage I/O. GC and
reconciliation consume one complete, bounded task projection collected from a
single immutable finalized query view; a truncated, drifting, malformed, or
unbound projection fails closed. A clean repair namespace with no status or
events returns a finalized-anchor-bound empty event page, while statusless
orphaned repair state still fails closed. Remaining work is deployed four-validator
proof of cross-peer exactly-once execution and restart reconciliation.
Operators must archive a production PoR/PoTR failure, repair, escalation,
appeal, and governance handoff from that reviewed deployment.
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
| Repair Scheduler | Accepts repair signals, creates tasks, drives workflow until closure. | Native repair state is authoritative; durable forwarders submit native transactions and reconcile finalized task/event queries. |
| Repair Worker | Executes bounded local rehydration, chunk fetch/re-seed, and orchestrator requests. | Execution is permitted only for the exact finalized live lease. The bounded in-process single-flight set is ephemeral coordination, never task state or authority. |
| Auditor API | Route-specific signed-transaction ingress for reports, escalations, appeals, and worker actions. | Torii accepts one caller-signed native transaction on each `/v1/sorafs/audit/repair/*` command route and performs exact instruction/action matching before strict durable ingress. |
| Payload Validator | Validates canonical report, slash-proposal, policy, approval, task-event, and audit-event payloads. | Implemented in `sorafs_manifest::repair` plus `sorafs_manifest::reference::validate_repair_payload_bytes`; transaction signatures, authority, permissions, revisions, leases, and idempotency are enforced by the common transaction/native-ISI path. |
| SLA & Telemetry | Metrics, logs, and alerts for backlog, latency, and outcomes. | Instrumented via `iroha_telemetry`, exported to OTLP + Prometheus. |
| Persistence | Durable recording of tasks, events, and outcomes. | Native ledger records and finalized task/event queries are the sole V1 authority. The only local durability is retry-safe signed-transaction forwarding and rebuildable consumer cursors. |
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

```

- `RepairTaskEventV1` is a bounded reference/publication payload; it is not the
  committed transition journal. The authoritative typed
  `SorafsRepairLedgerEvent` contains the ticket and task identities, provider,
  manifest, resulting revision, transaction authority, transition kind, and
  committing-block timestamp, without a free-form message.
- Command authentication uses the canonical Iroha `SignedTransaction` envelope.
  `/report` requires `SubmitSorafsRepairTask`; `/slash`, `/claim`,
  `/heartbeat`, `/complete`, and `/fail` require the matching
  `ApplySorafsRepairTaskAction` variant; `/appeal` requires
  `SubmitSorafsRepairAppeal`. Deleted pre-release
  `SignedAuditorRequestV1` and `RepairWorkerSignaturePayloadV1` envelopes are
  neither accepted nor retained as compatibility formats.
- `RepairAuditEventV1` and `GcAuditEventV1` wrap payloads with deterministic ordering metadata plus signer/digest fields for governance audit trails. Envelope validation is mandatory before publication: sequence numbers are non-zero, header and payload timestamps are identical, repair signers equal the payload actor (or the canonical `sorafs-repair` fallback), and GC signers equal `sorafs-gc`. `payload_digest` is BLAKE3 over the canonical header-bearing `norito::to_bytes(payload)` archive; bare codec payloads are never accepted as the digest preimage.
- GC audit payloads use the closed first-release reason vocabulary `retention_expired` or `retention_expired_provider_missing`; only the latter permits an all-zero provider identifier. Blocked outcomes use exactly `repair_active`, `deal_active`, or `shared_chunks` and must report zero freed bytes. Successful evictions may also report zero for valid empty manifests. Unknown labels or inconsistent provider/outcome fields fail before any governance artifact is written.

`RepairTaskStateV1` remains a reference payload for fixture and publication
validation. Native `RepairLedgerTaskV1` state plus the append-only
`SorafsRepairLedgerEvent` journal are the V1 execution authority.

## Scheduler Flow
1. **Triggers**
   - PoR coordinator publishes `PorFailureEventV1`.
   - PoTR probes emit `PotrDeadlineMissedV1`.
   - Auditors call `/v1/sorafs/audit/repair/report`.

2. **Payload Validation**
   - `RepairEvidenceV1`, reports, slash proposals, escalation approvals, task
     events, and audit events are validated through `sorafs_manifest::repair`
     and `sorafs_manifest::reference::validate_repair_payload_bytes`.
   - Torii rejects a command unless the signed transaction contains exactly one
     route-matching native instruction. Native execution then enforces
     authority, permissions, expected revision, lease generation, bounded
     canonical payloads, and exact idempotency before any committed mutation.

3. **Task Creation**
   - The producer derives one deterministic source identity and task identity,
     then submits `SubmitSorafsRepairTask` in a caller-signed transaction.
     Native execution admits one authoritative task, treats an exact replay as
     idempotent, and rejects conflicting reuse.
   - SLA and retry policy inputs are bounded native instruction fields and
     governed `iroha_config` values; process-local scheduler defaults cannot
     alter committed task state.

4. **Worker Assignment**
   - A worker reads the exact finalized revision, submits a caller-signed
     `Claim` action, and reconciles the committed lease owner, generation, and
     expiry before doing storage work.
   - Renew, complete, and fail transactions carry the expected revision and
     exact lease generation. Native execution rejects stale, expired,
     non-owner, and replay-conflicting actions.

5. **Failure & Escalation**
   - Failure and escalation are explicit native actions, not local watchdog
     mutations. Expected revision, live lease generation, idempotency key, and
     bounded evidence or failure digests are committed atomically.
   - Finalized escalation state carries the slash proposal material consumed by
     separately governed reserve, reputation, transparency, and Governance DAG
     producers.

6. **Governance Decision**
   - The provider owner submits `SubmitSorafsRepairAppeal` against the exact
     escalated task revision. Appeal state is committed on-chain and is visible
     through the finalized task and event projections.
   - `RepairEscalationApprovalV1` is a bounded governance
     publication/reference payload. It cannot mutate the task or replace the
     native appeal record.

7. **Queue Hygiene**
   - Expired leases become eligible for a new native claim; no local watchdog
     may rewrite lease or terminal state.
   - Supervised workers rebuild queues from finalized task/event queries and
     emit bounded metrics. Restart reconciliation must prove that duplicate
     submissions across peers still yield one lease and one terminal outcome.

## Governance Escalation Policy
Repair authority and policy are not sourced from a file key or environment
override. The obsolete `governance.sorafs_repair_escalation` configuration and
local vote/scheduler enforcement have been removed. Native instructions enforce
the registered provider-scoped worker authority, exact live lease, expected
revision, canonical slash-proposal provenance, and provider-owner appeal
authority. Any later custody slash or disbursement is a separately governed
native transition consuming the finalized escalation/appeal record.

`RepairEscalationPolicyV1` and `RepairEscalationApprovalV1` remain bounded
canonical publication/reference envelopes for governance evidence and SDK
validation. They cannot mutate a repair task, replace the committed appeal
record, or confer transaction authority.

## Auditor API Surface
| Method & Path | Description | Auth | Success Response |
|---------------|-------------|------|------------------|
| `POST /v1/sorafs/audit/repair/report` | Submit one caller-signed transaction containing `SubmitSorafsRepairTask`. | Transaction authority + native permission checks | `202 Accepted` from strict durable transaction ingress. |
| `POST /v1/sorafs/audit/repair/slash` | Submit one caller-signed transaction containing `ApplySorafsRepairTaskAction::Escalate`. | Active lease authority + native permission checks | `202 Accepted` from strict durable transaction ingress. |
| `POST /v1/sorafs/audit/repair/appeal` | Submit one caller-signed transaction containing `SubmitSorafsRepairAppeal`. | Provider-owner authority | `202 Accepted` from strict durable transaction ingress. |
| `GET /v1/sorafs/audit/repair/status` | Fetch authoritative repair counters at an optional exact finalized anchor. | Query perimeter | `200 OK` finalized projection; stale anchor is `409 Conflict`. |
| `GET /v1/sorafs/audit/repair/tasks` | Page authoritative tasks with an immutable exclusive task-id cursor and exact finalized anchor. | Query perimeter | `200 OK` finalized projection. |
| `GET /v1/sorafs/audit/repair/tasks/{ticket_id}` | Fetch one authoritative task, including lease, terminal, slash, appeal, revision, and receipts. | Query perimeter | `200 OK` finalized projection. |
| `GET /v1/sorafs/audit/repair/events` | Page typed payload-free committed events with an exclusive four-field cursor, exact finalized anchor, and ETag. | Query perimeter | `200 OK` finalized projection. |

## Worker API Surface
| Method & Path | Description | Auth | Success Response |
|---------------|-------------|------|------------------|
| `POST /v1/sorafs/audit/repair/claim` | Submit `ApplySorafsRepairTaskAction::Claim` with expected revision, bounded lease duration, and idempotency key. | Signed transaction + `CanOperateSorafsRepair` | `202 Accepted`. |
| `POST /v1/sorafs/audit/repair/heartbeat` | Submit `ApplySorafsRepairTaskAction::Renew` with expected revision, exact lease generation, bounded duration, and idempotency key. | Current lease owner + `CanOperateSorafsRepair` | `202 Accepted`. |
| `POST /v1/sorafs/audit/repair/complete` | Submit `ApplySorafsRepairTaskAction::Complete` with expected revision, exact lease generation, evidence digest, and idempotency key. | Current lease owner + `CanOperateSorafsRepair` | `202 Accepted`. |
| `POST /v1/sorafs/audit/repair/fail` | Submit `ApplySorafsRepairTaskAction::Fail` with expected revision, exact lease generation, failure digest, and idempotency key. | Current lease owner + `CanOperateSorafsRepair` | `202 Accepted`. |

### Repair Lease & Idempotency
- Lease expiry is derived from finalized committing-block time plus the bounded
  requested duration. Renew/complete/fail/escalate require the exact active
  lease generation and expected task revision.
- Native execution rejects stale revisions, non-owner actions, expired or
  mismatched leases, premature retry claims, and idempotency-key reuse with
  different canonical action bytes.

### Authentication
- Every command uses the common Iroha signed-transaction envelope; transaction
  authority is the only caller identity admitted by the route.
- Report admission requires the governed repair submitter permission.
  Claim/renew/complete/fail/escalate additionally enforce provider-scoped
  `CanOperateSorafsRepair`, task revision, and lease ownership in native
  execution. Appeals require the committed provider owner. There is no
  admin-only repair override.
- Dashboard or service integrations must construct and sign the same native
  transaction as CLI/API clients. Torii does not inject an authority or wrap an
  unsigned repair body.

### Rate Limiting & Replay Protection
- Torii applies the common transaction-ingress perimeter and replay controls
  before submission. The ledger transaction nonce/hash plus instruction-level
  source identities, revisions, lease generations, and idempotency receipts
  provide durable replay/equivocation protection.
- Route-specific local nonce or rate-limit state is not authoritative and
  cannot make an otherwise invalid ledger transition succeed.

## Validation Pipeline
- Implemented validation covers canonical Norito decoding, schema versions,
  non-zero digests and provider identifiers, timestamps, SLA and escalation
  policy, exact route/instruction matching, signed-transaction verification,
  authority/permission checks, revisions, leases, and idempotency.
- `sorafs-validate repair` exposes the same checks for fixture and release
  validation across repair evidence, reports, task records, slash proposals,
  escalation policy/approval payloads, task events, and audit events.
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
  - Runbook links to `specs/sorafs_ops_playbook.md` and `sorafs_gateway_self_cert.md` for transport-related incidents.

## Persistence & Retention
- The native ledger is the production persistence boundary for repair task,
  lease, terminal, slash, appeal, and event state.
- The native journal is contiguous and bounded-query consumers persist only
  their exclusive finalized cursor and rebuildable execution state. A complete
  production projection must be recoverable from finalized task/event queries
  and may never advance its cursor before the corresponding local side effect
  is durable.
- The retired `repair_state.to`/`FileRepairStore` format has no loader,
  migration, or compatibility branch. Pre-release state is discarded and
  reseeded; recovery rebuilds from finalized native queries.
- Durable transaction forwarders use bounded atomic checkpoints, exact
  signed-transaction bytes, retry/dead-letter state, and finalized
  reconciliation. Corrupt, truncated, trailing, unsafe-path, hard-linked,
  oversized, stale-writer, or post-rename durability failures fail closed.
- Governance audit publications remain append-only in the DAG, but they do not
  replace the native task, lease, terminal, slash, or appeal records.

## CLI & Torii Integration
- Command clients construct one route-specific native instruction, sign the
  containing Iroha transaction with the actual authority account, and submit
  the exact signed bytes. Torii never fills in authority, revision, lease
  generation, or idempotency data.
- Repair inspection uses the finalized `/status`, `/tasks`,
  `/tasks/{ticket_id}`, and `/events` queries. Clients pin
  `expected_finalized_height` plus `expected_finalized_block_hash_hex` across a
  scan and restart on `409 Conflict`.
- Worker tooling must read the committed task revision and lease generation
  before signing claim, renew, complete, fail, or escalate transactions. A
  retry reuses the exact signed transaction and idempotency key.
- `iroha sorafs gc inspect`: reports retained manifests and retention deadlines (read-only).
- `iroha sorafs gc dry-run`: reports only expired manifests that GC would evict (read-only).
- Torii exposes only the finalized, cursor-bounded committed-event polling
  route. The deleted local SSE and WebSocket routes are not V1 compatibility
  aliases.
- The generated Torii OpenAPI document advertises the signed-transaction
  request contract, `202 Accepted` ingress response, finalized anchor pairs,
  immutable task/event cursors, and the absence of obsolete local routes.

## Governance & Escalation
- Finalized repair escalation and appeal events are the only valid inputs to
  Reserve+Rent, reputation, transparency, and Governance DAG handoff workers.
  Each worker must use a durable cursor and idempotency key, submit any custody
  mutation as its own governed native transaction, and reconcile committed
  state.
- Automatic collateral mutation and automatic DAG publication are not implied
  by the repair instruction. Production promotion remains blocked until these
  finalized-event consumers and their retry/recovery evidence are deployed.
- Weekly governance meeting reviews:
  - Total repairs, escalations, penalties.
  - Auditor performance (response time, quality).
  - Outstanding proposals older than 7 days (must be decided or escalated).

### Escalation policy boundary

No local defaults authorize a proposal, vote, slash, refund, or disbursement.
Slash proposals carrying an embedded approval summary fail closed. Governance
consumers must bind their current policy/digest anchor to the finalized native
escalation and appeal records, apply their own authority and custody checks,
and reconcile the resulting committed transition.

## Rollout Evidence Gate

Use the rollout gate only after exact-live-lease execution and restart
reconciliation have been proved in the reviewed deployment, and the deployed
auditor roster, SF-9 coordinator, PoR/PoTR failure capture,
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
The closed route/status map requires `202 Accepted` for report, slash, appeal,
claim, heartbeat, complete, and fail commands, and `200 OK` for status, task
list, single-task, and committed-event queries. The `auditor_api` kind name and
`raw_auditor_request` redaction key are stable evidence-schema identifiers;
neither denotes the deleted `SignedAuditorRequestV1` wire envelope.
Auditor-roster artifacts also bind `auditor_count` to the unique canonical
`auditors[].name` inventory, require reviewed `repair-auditor-*` labels without
non-production markers, and reject duplicate auditor entries before promotion
can report ready.
Failure-capture artifacts must also keep `failure_source_count` equal to the
unique canonical `failure_sources` inventory, keep `failure_event_count` equal
to the unique canonical `failure_events[].name` inventory, require reviewed
`repair-failure-event-*` labels without non-production markers, require
reviewed failure events to cover both PoR and PoTR sources, and reject
duplicate or unknown source entries plus duplicate event entries. Worker
lifecycle artifacts require `status_count`, bind it to the unique canonical
`statuses_observed` inventory, reject missing, inflated, duplicate, or unknown
lifecycle-status evidence, and require finalized task projection, exact live
lease execution, durable transaction forwarding, restart reconciliation, and a
single terminal outcome. Governance handoff artifacts require
`handoff_target_count`, bind it
to the unique canonical `handoff_targets` inventory, and reject missing,
inflated, duplicate, or unknown handoff-target evidence. Observability artifacts
also bind `metric_count` to the unique canonical `metrics` inventory, require
the reviewed repair metrics, and reject duplicate or unknown metric labels
before promotion can report ready.
The canary builder only encodes these reviewed operator assertions in the
closed payload-free schema; generating a canary does not prove them. Promotion
requires genuine signed artifacts from the reviewed deployment, and reviewers
must validate the underlying finalized cursors, transaction receipts, restart
records, and cross-peer terminal outcome outside the payload-free bundle.
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
- Native records/ISIs own task admission, compare-and-set lease actions,
  terminal outcomes, slash proposals, appeals, action receipts, status, and
  committed-event sequencing.
- Torii command routes accept exactly one matching caller-signed native
  transaction and forward through strict durable ingress; query routes return
  finalized projections with exact anchors and immutable cursors.
- Durable repair forwarders and finalized-lease-gated storage execution cover
  retry, response loss, restart, stale/wrong lease, and reconciliation
  failures. Local golden and integration coverage exercises native authority,
  route matching, query cursors, and cross-peer duplicate submission.
- `iroha sorafs repair` and `iroha sorafs gc` provide operator CLI coverage;
  `sorafs-validate repair` validates the remaining canonical manifest repair
  payloads, not deleted custom request envelopes.
- `scripts/build_sorafs_repair_canary.py` provides checked-in payload-free
  canary generation for the local SF-8b rollout gate.
- The SF-8b rollout evidence gate, collection planner, operator argfile
  templates, and focused tests are implemented for payload-free deployed
  evidence review, including cross-artifact auditor-roster and failure-bundle
  digest binding plus governance handoff digest and handoff policy digest
  binding.

The competing local repair authority and GC/reconciliation checkpoint
dependencies are removed. Remaining rollout work is genuine four-validator
evidence for a production PoR/PoTR failure, one cross-peer lease and terminal
outcome, escalation/appeal, restart reconciliation, and governance handoff,
followed by the SF-8b rollout evidence gate.
