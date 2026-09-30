# __SORACLOUD_APP_NAME__ API Service

This deterministic IVM service owns the root-bound app API.

The starter contract exposes:

- `GET /api/healthz`

## Local dev shim

```bash
./dev.sh
```

`dev.sh` starts a local HTTP shim for `/api/healthz` so the root-bound frontend
can be exercised without a live deterministic runtime. Build and deploy still
use the IVM bytecode emitted by `./build.sh`.

## Build

```bash
KOTO_BIN=/absolute/path/to/koto KOTO_BIN_SHA256=<lowercase-sha256> ./build.sh
```

`KOTO_BIN` must be an absolute, non-symlinked same-revision binary and
`KOTO_BIN_SHA256` must match it; the script never searches `PATH` or compiles a
source fallback.

The build emits `build/api-service.to` plus
`build/api-service.contract_manifest.json`.

## Offline verification

```bash
KOTO_BIN=/absolute/path/to/koto ./verify-build.sh
```

`verify-build.sh` recompiles `contract/api_service.ko` with `koto build` and
checks that both the bytecode and emitted contract manifest still match the
committed build outputs.

The generated app manifest already
points at that bytecode through `bundle_file`, so:

```bash
iroha soracloud service sync-manifests --app-manifest ../../app_manifest.json
```

refreshes the container bundle hash and the service container manifest hash in
one pass before deploy.
