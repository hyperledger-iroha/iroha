# Soracloud CLI and Control Plane

Soracloud v1 is an authoritative, IVM-only runtime.

- `iroha app soracloud init` is the only offline command. It scaffolds
  `container_manifest.json`, `service_manifest.json`, and optional template
  artifacts for Soracloud services.
- All other Soracloud CLI commands are network-backed only and require
  `--torii-url`.
- The CLI does not maintain any local Soracloud control-plane mirror or state
  file.
- Torii serves public Soracloud status and mutation routes directly from
  authoritative world state plus the embedded Soracloud runtime manager.

## Runtime Scope

- Soracloud v1 accepts only `SoraContainerRuntimeV1::Ivm`.
- `NativeProcess` remains rejected.
- Ordered mailbox execution runs admitted IVM handlers directly.
- Hydration and materialization come from committed SoraFS/DA content rather
  than synthetic local snapshots.

## CLI Commands

- `iroha app soracloud init`
  - offline scaffold only.
  - supports `baseline`, `site`, `webapp`, and `pii-app` templates.
- `iroha app soracloud deploy`
  - validates `SoraDeploymentBundleV1` admission rules locally, signs the
    request, and calls `POST /v1/soracloud/deploy`.
  - Torii also enforces SCR-host admission caps and fail-closed capability
    checks before the mutation is accepted.
- `iroha app soracloud upgrade`
  - validates and signs a new bundle revision, then calls
    `POST /v1/soracloud/upgrade`.
  - The same SCR-host admission checks run server-side before the upgrade is
    admitted.
- `iroha app soracloud status`
  - queries authoritative service status from `GET /v1/soracloud/status`.
- `iroha app soracloud rollback`
  - signs rollback metadata and calls `POST /v1/soracloud/rollback`.
- `iroha app soracloud rollout`
  - signs rollout metadata and calls `POST /v1/soracloud/rollout`.
- `iroha app soracloud agent-*`
  - all apartment lifecycle, wallet, mailbox, and autonomy commands are
    Torii-backed only.
- `iroha app soracloud training-*`
  - all training job commands are Torii-backed only.
- `iroha app soracloud model-*`
  - all model artifact and weight commands are Torii-backed only.
- `iroha app soracloud hf-*`
  - `hf-deploy`, `hf-status`, `hf-lease-leave`, and `hf-lease-renew` are
    Torii-backed only.
  - these commands manage authoritative shared Hugging Face lease
    source/pool/member state and request signatures; they do not yet perform
    async Hub import, runtime hydration, or inference-service/apartment
    bring-up.

## Status Semantics

`/v1/soracloud/status` and the related agent/training/model status endpoints now
reflect authoritative runtime state:

- admitted service revisions from committed world state;
- runtime hydration/materialization state from the embedded runtime manager;
- real mailbox execution receipts and failure state;
- published journal/checkpoint artifacts;
- cache and runtime health instead of placeholder status shims.

If authoritative runtime material is stale or unavailable, reads fail closed
instead of falling back to local state mirrors.

`/v1/soracloud/status` is the only documented Soracloud status endpoint in v1.
There is no separate `/v1/soracloud/registry` route.

## Removed Local Scaffolding

These older local-simulation concepts no longer exist in v1:

- CLI-local registry/state files or registry-path options
- Torii-local file-backed control-plane mirrors

## Example

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## Notes

- Local validation still runs before requests are signed and submitted.
- Private-runtime capability enforcement happens inside the Soracloud host ABI,
  not inside CLI or Torii-local scaffolding.
- Runtime health, hydration, and execution are sourced from
  `[soracloud_runtime]` configuration and committed state, not environment
  toggles.
