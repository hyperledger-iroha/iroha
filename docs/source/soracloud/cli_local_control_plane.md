# Soracloud CLI Local Control Plane

`iroha app soracloud` provides:

- a deterministic local simulation for Soracloud control-plane workflows; and
- network-backed control-plane mode that queries and mutates live Torii
  endpoints.

## Commands

- `iroha app soracloud init`
  - scaffolds `container_manifest.json`, `service_manifest.json`, and
    `registry.json`.
  - supports developer templates:
    - `--template baseline` (manifests only, default)
    - `--template site` (Vue3/Vite static SPA + SoraFS/SoraDNS workflow files)
    - `--template webapp` (Vue3 SPA + API starter with session/auth + chain-ID hooks)
    - `--template health-app` (private health workload starter: Vue3 + API + consent/retention/deletion policy templates)
- `iroha app soracloud deploy`
  - validates a deployment bundle and registers a new service revision.
  - when `--torii-url` is supplied, signs the bundle payload with the
    configured client keypair and calls `POST /v1/soracloud/deploy`.
- `iroha app soracloud status`
  - prints machine-readable registry status (local mode by default).
  - when `--torii-url` is supplied, fetches a live control-plane snapshot from
    `GET /v1/soracloud/status`.
- `iroha app soracloud upgrade`
  - validates manifests and appends an upgrade revision.
  - when `--torii-url` is supplied, signs the bundle payload with the
    configured client keypair and calls `POST /v1/soracloud/upgrade`.
- `iroha app soracloud rollback`
  - switches service state to a previous (or explicit) version and records an
    audit event.
  - when `--torii-url` is supplied, signs rollback payload metadata and calls
    `POST /v1/soracloud/rollback`.
- `iroha app soracloud rollout`
  - advances or fails a rollout step for an active canary handle.
  - accepts health signals (`--health healthy|unhealthy`) and optional
    traffic promotion (`--promote-to-percent`).
  - records/forwards `--governance-tx-hash` for deterministic audit linkage.
  - when `--torii-url` is supplied, signs rollout metadata and calls
    `POST /v1/soracloud/rollout`.
- `iroha app soracloud agent-deploy`
  - validates `AgentApartmentManifestV1` and registers apartment runtime state.
  - initializes lease expiry (`--lease-ticks`) and append-only apartment events.
  - when `--torii-url` is supplied, signs payload metadata and calls
    `POST /v1/soracloud/agent/deploy`.
- `iroha app soracloud agent-lease-renew`
  - extends lease duration for an existing apartment.
  - revives expired leases into `Running` status in local scheduler state.
- `iroha app soracloud agent-restart`
  - requests deterministic restart for an apartment with an active lease.
  - records restart reason and restart sequence in scheduler events.
- `iroha app soracloud agent-status`
  - prints apartment scheduler snapshot (status, lease remaining, quotas/policy counts).
- `iroha app soracloud agent-wallet-spend`
  - submits an apartment wallet spend request under deterministic policy checks.
  - enforces `wallet.sign` capability + per-asset `max_per_tx_nanos`/`max_per_day_nanos`.
  - auto-approves only when `wallet.auto_approve` capability is active.
- `iroha app soracloud agent-wallet-approve`
  - approves a pending wallet request by `request_id`.
  - records spend into deterministic day buckets (`sequence / 10_000`).
- `iroha app soracloud agent-policy-revoke`
  - revokes a declared apartment policy capability (for example `wallet.sign`).
  - updates runtime guardrails without mutating the original manifest payload.
  - when `--torii-url` is supplied, signs payload metadata and calls
    `POST /v1/soracloud/agent/policy/revoke`.
- `iroha app soracloud agent-message-send`
  - enqueues a deterministic mailbox message to another apartment.
  - requires active `agent.mailbox.send` on sender and `agent.mailbox.receive` on recipient.
- `iroha app soracloud agent-message-ack`
  - consumes a queued mailbox message by `message_id`.
  - requires active `agent.mailbox.receive` on the recipient apartment.
- `iroha app soracloud agent-mailbox-status`
  - shows pending mailbox messages, message hashes, and queue depth for one apartment.
- `iroha app soracloud agent-artifact-allow`
  - adds/updates an autonomy artifact allowlist entry (`artifact_hash` +
    optional required `provenance_hash`) for one apartment.
  - requires active `governance.audit` or `agent.autonomy.allow`.
  - when `--torii-url` is supplied, signs payload metadata and calls
    `POST /v1/soracloud/agent/autonomy/allow`.
- `iroha app soracloud agent-autonomy-run`
  - approves a deterministic autonomous run for an allowlisted artifact.
  - requires active `agent.autonomy.run`, allowlist match, optional provenance
    match, and available budget (`--budget-units`).
  - when `--torii-url` is supplied, signs payload metadata and calls
    `POST /v1/soracloud/agent/autonomy/run`.
- `iroha app soracloud agent-autonomy-status`
  - shows autonomy budget ceiling/remaining, artifact allowlist entries, and
    recent autonomous run approvals for one apartment.
  - when `--torii-url` is supplied, queries
    `GET /v1/soracloud/agent/autonomy/status`.
- `iroha app soracloud training-job-start`
  - starts a signed distributed training job (`service_name`, `model_name`,
    `job_id`, worker/budget/checkpoint controls).
  - requires `--torii-url`; local simulation mode does not implement this flow.
  - calls `POST /v1/soracloud/training/job/start`.
- `iroha app soracloud training-job-checkpoint`
  - submits a signed monotonic checkpoint update (`completed_step`,
    `checkpoint_size_bytes`, `metrics_hash`).
  - requires `--torii-url`.
  - calls `POST /v1/soracloud/training/job/checkpoint`.
- `iroha app soracloud training-job-retry`
  - submits a signed retry request with an explicit reason string.
  - requires `--torii-url`.
  - calls `POST /v1/soracloud/training/job/retry`.
- `iroha app soracloud training-job-status`
  - queries one training job status entry by `service_name` + `job_id`.
  - requires `--torii-url`.
  - calls `GET /v1/soracloud/training/job/status`.
- `iroha app soracloud model-artifact-register`
  - submits signed artifact-attestation metadata for a completed training job
    (`weight_artifact_hash`, `dataset_ref`, config/repro/provenance hashes).
  - requires `--torii-url`.
  - calls `POST /v1/soracloud/model/artifact/register`.
- `iroha app soracloud model-artifact-status`
  - queries artifact-attestation status by `service_name` + `training_job_id`.
  - requires `--torii-url`.
  - calls `GET /v1/soracloud/model/artifact/status`.
- `iroha app soracloud model-weight-register`
  - submits a signed model weight version registration (optional
    `--parent-version`) linked to a completed/attested training job.
  - requires `--torii-url`.
  - calls `POST /v1/soracloud/model/weight/register`.
- `iroha app soracloud model-weight-promote`
  - submits a signed promotion decision with `--gate-approved` and
    `--gate-report-hash`.
  - requires `--torii-url`.
  - calls `POST /v1/soracloud/model/weight/promote`.
- `iroha app soracloud model-weight-rollback`
  - submits a signed rollback decision to an already-registered target version.
  - requires `--torii-url`.
  - calls `POST /v1/soracloud/model/weight/rollback`.
- `iroha app soracloud model-weight-status`
  - queries model lineage/current-version status by `service_name` +
    `model_name`.
  - requires `--torii-url`.
  - calls `GET /v1/soracloud/model/weight/status`.

## Deterministic admission checks

Deploy/upgrade commands run `SoraDeploymentBundleV1::validate_for_admission()`
before mutating state, including:

- container/service schema compatibility;
- container hash linkage (`service.container.manifest_hash`);
- mutable state-binding capability checks;
- public-route healthcheck requirements.

## Registry format

The default registry path is `.soracloud/registry.json`. The state keeps:

- `services`: per-service current version and revision history;
- `services[].active_rollout`/`services[].last_rollout`: rollout runtime state
  (handle, stage, canary traffic, health failures, policy limits);
- `audit_log`: append-only deploy/upgrade/rollback/rollout records with
  sequence ids, optional rollout handles, and optional governance tx hash
  references;
- `apartments`: persistent apartment runtime state keyed by apartment name
  (manifest hash, lease window, restart metadata, revoked policy capabilities,
  pending wallet requests, daily spend accumulators, mailbox queues, autonomy
  budget counters, artifact allowlist, and autonomous run history);
- `apartment_events`: append-only deploy/lease-renew/restart events with
  deterministic sequence ids (including wallet spend request/approve, policy revoke,
  mailbox enqueue/acknowledge, artifact allow, and autonomy run approval
  events).

## Network-backed mode

Use Torii control-plane APIs instead of local registry simulation:

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

```bash
iroha app soracloud rollback \
  --service-name web_portal \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

```bash
iroha app soracloud rollout \
  --service-name web_portal \
  --rollout-handle web_portal:rollout:2 \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

```bash
iroha app soracloud status \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

The CLI output includes `source: "torii_control_plane"` and embeds
`network_status` from Torii, including:

- `schema_version`
- `service_health`
- `routing`
- `resource_pressure`
- `failed_admissions`
- `control_plane` (registry/audit snapshot)

## Templates

Generate a static site starter:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --output-dir .soracloud-docs
```

Generate a dynamic webapp starter:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --output-dir .soracloud-agent
```

Generate a regulated health workload starter:

```bash
iroha app soracloud init \
  --template health-app \
  --service-name clinic_console \
  --output-dir .soracloud-health
```

Both templates keep IVM/SCR assumptions (no WASM runtime dependency) and emit
deterministic starter artifacts that can be versioned in CI:

- `site/`, `webapp/`, or `health-app/` source tree (Vue3 + API files);
- canonical Soracloud manifests (`container_manifest.json`, `service_manifest.json`);
- `registry.json` for local control-plane simulation.
- health-app policy templates (`policy/consent_policy_template.json`,
  `policy/retention_policy_template.json`,
  `policy/deletion_workflow_template.json`) for consent and retention/deletion governance workflows.

## Deterministic state-binding guardrail API

Torii now exposes a signed control-plane mutation path:

- `POST /v1/soracloud/state/mutate`

This endpoint is intended for SCR-hosted services to request canonical state
mutations under declared `state_bindings` and enforces:

- binding existence for the active service revision;
- key-prefix confinement (`binding.key_prefix`);
- mutability policy (`ReadOnly`, `AppendOnly`, `ReadWrite`);
- encryption-policy match (`request.encryption == binding.encryption`);
- `max_item_bytes` and `max_total_bytes` quota limits.

Each accepted mutation appends an audit event carrying signer identity and
`governance_tx_hash` linkage for deterministic policy review.

## SCR host admission + lifecycle

`deploy`/`upgrade`/`rollback` admission now applies deterministic SCR host
policy checks before mutating service registry state:

- bounded resource caps for `cpu_millis`, `memory_bytes`,
  `ephemeral_storage_bytes`, `max_open_files`, and `max_tasks`;
- bounded lifecycle caps for `start_grace_secs` and `stop_grace_secs`;
- capability-policy compatibility checks (for example,
  `allow_state_writes=false` rejects non-readonly state bindings);
- validated network allowlist policy (non-empty, non-duplicate host entries);
- deterministic sandbox profile hashing from runtime/capability/resource/lifecycle
  policy (`sandbox_profile_hash`).

Each admitted service revision records SCR host snapshot fields in
`latest_revision` status metadata, including runtime kind, capability flags,
resource/lifecycle limits, and monotonic process-lifecycle markers
(`process_generation`, `process_started_sequence`) across
deploy/upgrade/rollback transitions.

## Health access-consent API

Torii exposes signed policy-gated disclosure endpoints for regulated workloads:

- `POST /v1/soracloud/decrypt/request`
- `POST /v1/soracloud/health/access/request` (health-oriented alias)

Both endpoints enforce `DecryptionAuthorityPolicyV1` + `DecryptionRequestV1`
admission constraints, including:

- strict jurisdiction-tag matching (`request.jurisdiction_tag == policy.jurisdiction_tag`);
- consent-evidence hash requirements for non-break-glass requests when
  `policy.require_consent_evidence=true`;
- break-glass gating + mandatory `break_glass_reason`;
- append-only registry audit entries with policy/jurisdiction/break-glass metadata.

## Health compliance report API

Torii also exposes a compliance report-pack endpoint for regulated workload
operations:

- `GET /v1/soracloud/health/compliance/report`

Supported query parameters:

- `service_name` (optional): scope report rows to one deployed service.
- `jurisdiction_tag` (optional): scope report rows to one jurisdiction.
- `limit` (optional): caps recent access entries and policy diff rows
  (default `50`, max `500`).

The report payload includes:

- access-log totals (`total_access_events`, break-glass/non-break-glass counts);
- consent-evidence completeness (`consent_evidence_present_events`,
  `consent_evidence_coverage_bps`);
- recent disclosure entries (`recent_access_events`);
- per-jurisdiction summary stats (`jurisdiction_stats`);
- encrypted-binding data-flow attestations (`data_flow_attestations`);
- policy snapshot diff history (`policy_diff_history`).

## Health privacy/security test matrix

The Torii Soracloud suite covers the minimum regulated-workload matrix:

- least-privilege scope enforcement:
  `soracloud::tests::health_privacy_matrix_enforces_declared_binding_scope`
- unauthorized-access rejection (tampered signature):
  `soracloud::tests::health_privacy_matrix_rejects_unauthorized_decryption_signatures`
- evidence completeness reporting:
  `soracloud::tests::health_privacy_matrix_reports_evidence_completeness_gaps`

## SCR training runtime API

Torii now exposes signed distributed-training runtime endpoints:

- `POST /v1/soracloud/training/job/start`
- `POST /v1/soracloud/training/job/checkpoint`
- `POST /v1/soracloud/training/job/retry`
- `GET /v1/soracloud/training/job/status?service_name=<name>&job_id=<id>`

The runtime enforces deterministic policy and accounting semantics:

- active service revision must enable `allow_model_training`;
- bounded worker-group size and retry count;
- monotonic checkpoint progression aligned to checkpoint interval or final step;
- deterministic compute/storage budget metering per checkpoint;
- append-only training audit log with signer linkage.

Core regressions:

- `soracloud::tests::training_job_lifecycle_tracks_checkpoints_retries_and_completion`
- `soracloud::tests::training_job_start_rejects_services_without_training_capability`

## SCR model/weight registry API

Torii also exposes signed model-weight lifecycle endpoints:

- `POST /v1/soracloud/model/weight/register`
- `POST /v1/soracloud/model/weight/promote`
- `POST /v1/soracloud/model/weight/rollback`
- `GET /v1/soracloud/model/weight/status?service_name=<name>&model_name=<name>`

Registry semantics:

- version-lineage enforcement (`parent_version` required for non-root versions);
- registration requires a completed training job for the same model;
- registration records artifact metadata (`dataset_ref`, `training_config_hash`,
  `reproducibility_hash`, `provenance_attestation_hash`) with each version;
- promotion requires explicit gate approval + gate report hash;
- rollback requires a previously registered target version and reason;
- append-only model-weight audit events with signer linkage.

Core regressions:

- `soracloud::tests::model_weight_lifecycle_supports_lineage_promotion_and_rollback`
- `soracloud::tests::model_weight_register_rejects_non_completed_training_jobs`

## SCR model artifact attestation API

Torii also exposes signed artifact-attestation endpoints for training outputs:

- `POST /v1/soracloud/model/artifact/register`
- `GET /v1/soracloud/model/artifact/status?service_name=<name>&training_job_id=<id>`

Pipeline semantics:

- attestation registration requires a completed training job for the same model;
- attestation records bind `weight_artifact_hash`, `dataset_ref`,
  `training_config_hash`, `reproducibility_hash`,
  `provenance_attestation_hash` to the training job;
- model-weight registration rejects missing attestation and any metadata/hash
  mismatches before version registration;
- successful model-weight registration consumes the attestation into the
  registered version lineage (`consumed_by_version`).

Core regressions:

- `soracloud::tests::secure_artifact_pipeline_requires_artifact_registration_before_weight_register`
- `soracloud::tests::secure_artifact_pipeline_enforces_metadata_match_and_consumption`

## SCR model-training benchmark coverage

Deterministic benchmark-style integration coverage for model-training runtime
paths is implemented in Torii Soracloud tests:

- `soracloud::tests::training_benchmark_throughput_and_checkpoint_recovery_are_deterministic`
  validates deterministic throughput accounting (`completed_steps` per event)
  and checkpoint recovery continuity after persisted registry reload.
- `soracloud::tests::promotion_policy_benchmark_gate_enforcement_is_deterministic`
  validates deterministic promotion policy behavior across repeated runs
  (gate rejection when `gate_approved=false`, promotion success when
  `gate_approved=true`).

## SCR autonomy runtime API

Torii now exposes signed autonomy runtime endpoints for SCR-side apartment
controls (beyond local CLI simulation):

- `POST /v1/soracloud/agent/deploy`
- `POST /v1/soracloud/agent/policy/revoke`
- `POST /v1/soracloud/agent/autonomy/allow`
- `POST /v1/soracloud/agent/autonomy/run`
- `GET /v1/soracloud/agent/autonomy/status?apartment_name=<name>`

These endpoints enforce deterministic runtime policy checks:

- apartment manifest validation + lease model at deploy time;
- capability revocation guardrails (`agent.autonomy.run`, etc.);
- artifact allowlist + optional provenance pin matching;
- autonomy budget accounting and non-negative budget enforcement;
- deterministic autonomy checkpoint persistence under
  `manifest.state_quota_bytes` with quota rejection on overflow;
- append-only apartment audit events with signer linkage.

Torii persists this runtime/control-plane state to:

- `torii.data_dir/soracloud/registry_state.to`

using atomic Norito snapshot writes (`*.tmp` then rename), so apartment queue
state (wallet approvals, mailbox messages, autonomy budget/history, lease
windows, process-generation lifecycle metadata, and persisted autonomy
checkpoint usage) survives peer restarts.

## Runbooks

- Vue3 SPA + API deployment and production operations:
  `docs/source/soracloud/vue3_spa_api_runbook.md`
