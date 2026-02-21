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
  references.

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
