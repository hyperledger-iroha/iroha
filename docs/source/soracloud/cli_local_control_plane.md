# Soracloud CLI Local Control Plane

`iroha app soracloud` provides:

- a deterministic local simulation for Soracloud control-plane workflows; and
- network-backed control-plane mode that queries and mutates live Torii
  endpoints.

## Commands

- `iroha app soracloud init`
  - scaffolds `container_manifest.json`, `service_manifest.json`, and
    `registry.json`.
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
- `audit_log`: append-only deploy/upgrade/rollback records with sequence ids.

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
