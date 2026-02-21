# Soracloud Vue3 SPA + API Runbook

This runbook covers production-oriented deployment and operations for:

- a Vue3 static site (`--template site`); and
- a Vue3 SPA + API service (`--template webapp`),

using Soracloud control-plane APIs on Iroha 3 with SCR/IVM assumptions (no
WASM runtime dependency and no Docker dependency).

## 1. Generate template projects

Static site scaffold:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API scaffold:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Each output directory includes:

- `container_manifest.json`
- `service_manifest.json`
- `registry.json` (local deterministic simulation)
- template source files under `site/` or `webapp/`

## 2. Build application artifacts

Static site:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA frontend + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. Package and publish frontend assets

For static hosting via SoraFS:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

For SPA frontend:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Deploy to live Soracloud control plane

Deploy static site service:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Deploy SPA + API service:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Validate route binding and rollout state:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Expected control-plane checks:

- `control_plane.services[].latest_revision.route_host` set
- `control_plane.services[].latest_revision.route_path_prefix` set (`/` or `/api`)
- `control_plane.services[].active_rollout` present immediately after upgrade

## 5. Upgrade with health-gated rollout

1. Bump `service_version` in the service manifest.
2. Run upgrade:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Promote rollout after health checks:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. If health fails, report unhealthy:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

When unhealthy reports reach policy threshold, Soracloud automatically rolls
back to baseline revision and records rollback audit events.

## 6. Manual rollback and incident response

Rollback to previous version:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Use status output to confirm:

- `current_version` reverted
- `audit_event_count` incremented
- `active_rollout` cleared
- `last_rollout.stage` is `RolledBack` for automatic rollbacks

## 7. Operations checklist

- Keep template-generated manifests under version control.
- Record `governance_tx_hash` for each rollout step to preserve traceability.
- Treat `service_health`, `routing`, `resource_pressure`, and
  `failed_admissions` as rollout gate inputs.
- Use canary percentages and explicit promotion rather than direct full-cut
  upgrades for user-facing services.
- Validate session/auth and signature-verification behavior in
  `webapp/api/server.mjs` before production.
