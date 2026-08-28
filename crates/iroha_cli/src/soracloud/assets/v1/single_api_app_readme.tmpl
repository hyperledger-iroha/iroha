# __SORACLOUD_APP_NAME__ Single-API App Template

This template provides:

- `web/` root-bound Vue frontend intended for publication on the app hostname
- `services/api/` deterministic IVM API bundle plus a local dev shim
- `app_manifest.json` wiring both surfaces together
- `dev.sh` to boot the frontend plus the local API shim
- `build-and-sync.sh` to rebuild every artifact and refresh manifest hashes
- `release.sh` to run the mandatory build, validation, publication, deploy,
  authoritative-status, and live-verification flow

## Local dev

```bash
./dev.sh
```

This starts the frontend dev server on `http://127.0.0.1:5173` and the API dev
shim on `http://127.0.0.1:8787`. The generated Vite config keeps `/api`
same-host and proxies it to the local API shim.

The CLI can resolve and run the same manifest-adjacent entrypoint:

```bash
iroha soracloud app dev --manifest ./app_manifest.json --dry-run
iroha soracloud app dev --manifest ./app_manifest.json
```

`iroha soracloud app plan --manifest ./app_manifest.json` also prints
the root `manifest_path`, root `hostname`, the resolved child manifest paths,
child `workspace_dir`, and child service scripts for service-scoped Soracloud
commands.

## Build everything

```bash
./build-and-sync.sh
iroha soracloud app build --manifest ./app_manifest.json --dry-run
iroha soracloud app build --manifest ./app_manifest.json
```

`build-and-sync.sh` runs the frontend build, `services/api/./build.sh`, and
`services/api/./verify-build.sh` before refreshing manifest hashes.

Root scripts require `IROHA_BIN` to be an absolute path to an
operator-qualified binary built from the same revision as the scaffold and
`IROHA_BIN_SHA256` to match that exact file. They never resolve `iroha` from
`PATH` or compile a source checkout while releasing. Contract build scripts
likewise require an absolute same-revision `KOTO_BIN` and matching
`KOTO_BIN_SHA256`.

The app manifest references `services/api/build/api-service.to`, so the app-wide
sync path updates the bundle hash automatically.

## Publish + deploy

```bash
SORAFS_RETENTION_EPOCH=2000000000 TORII_URL=http://127.0.0.1:8080 ./release.sh
iroha soracloud app doctor --manifest ./app_manifest.json
iroha soracloud app release --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --sorafs-retention-epoch 2000000000 --dry-run
iroha soracloud app release --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --sorafs-retention-epoch 2000000000
```

Choose a future Unix-second SoraFS retention boundary for the release and
reuse that exact value on every retry. It is part of each manifest identity.

`app release` is the only first-release app mutation command. It does not
accept a build bypass, and it returns overall success only after authoritative
status reconciliation and an exact live 2xx route/health probe.

## Inspect deployed status

```bash
iroha soracloud app status --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080
```

`app status` keeps one entry per child service manifest and reports the child
manifest paths, the root `manifest_path`, root `hostname`, root
`workspace_dir`, root `workspace_scripts`, plane/runtime, route prefix, the
top-level `routes` split, the frontend publish projection, and the
matched Torii control-plane status when present.

Service-scoped Soracloud commands still operate on the child API manifests at
`services/api/container_manifest.json` plus `services/api/service_manifest.json`.

When those commands are driven by `--container` plus `--service`, their
responses also attach the same local `service_plan` projection that
`iroha soracloud service plan` reports.

The frontend stays bound at `https://__SORACLOUD_PACKAGE_NAME__.sora/`, and the API stays on
the same host under `https://__SORACLOUD_PACKAGE_NAME__.sora/api/healthz`.
