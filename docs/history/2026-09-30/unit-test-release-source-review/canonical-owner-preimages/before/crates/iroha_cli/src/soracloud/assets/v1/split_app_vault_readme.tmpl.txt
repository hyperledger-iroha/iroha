# __SORACLOUD_APP_NAME__ Vault API

This service is the deterministic IVM plane for wallet auth and private user
state.

## Local dev shim

```bash
./dev.sh
```

`dev.sh` starts a local HTTP shim for `/api/auth*` and `/api/v1/user*` routes so
the split app can be exercised without a live deterministic runtime. Build and
deploy still use the IVM bytecode emitted by `./build.sh`.

## Local verification

```bash
KOTO_BIN=/absolute/path/to/koto KOTO_BIN_SHA256=<lowercase-sha256> ./build.sh
KOTO_BIN=/absolute/path/to/koto KOTO_BIN_SHA256=<lowercase-sha256> ./verify-build.sh
```

`verify-build.sh` recompiles `contract/vault_api.ko` with `koto build` and
checks that both the bytecode and emitted contract manifest match the committed
build outputs. `KOTO_BIN` must be an absolute, non-symlinked same-revision
binary and `KOTO_BIN_SHA256` must match it; neither script searches `PATH` or
compiles source fallback.

Build:

```bash
KOTO_BIN=/absolute/path/to/koto KOTO_BIN_SHA256=<lowercase-sha256> ./build.sh
```

The build emits `build/vault-api.to`, and the generated app manifest references
that bytecode through `bundle_file` so the app-wide `sync-manifests` command can
refresh the admitted manifest hashes in one pass.
