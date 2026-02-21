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
- `iroha app soracloud agent-autonomy-run`
  - approves a deterministic autonomous run for an allowlisted artifact.
  - requires active `agent.autonomy.run`, allowlist match, optional provenance
    match, and available budget (`--budget-units`).
- `iroha app soracloud agent-autonomy-status`
  - shows autonomy budget ceiling/remaining, artifact allowlist entries, and
    recent autonomous run approvals for one apartment.

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

## Vue3 Templates

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

Both templates keep IVM/SCR assumptions (no WASM runtime dependency) and emit
deterministic starter artifacts that can be versioned in CI:

- `site/` or `webapp/` source tree (Vue3 + API files);
- canonical Soracloud manifests (`container_manifest.json`, `service_manifest.json`);
- `registry.json` for local control-plane simulation.

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
- append-only apartment audit events with signer linkage.

## Runbooks

- Vue3 SPA + API deployment and production operations:
  `docs/source/soracloud/vue3_spa_api_runbook.md`
