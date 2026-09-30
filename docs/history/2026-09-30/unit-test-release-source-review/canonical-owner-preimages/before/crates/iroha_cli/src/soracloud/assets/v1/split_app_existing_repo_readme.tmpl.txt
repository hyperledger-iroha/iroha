# __SORACLOUD_APP_NAME__ Split App Existing-Repo Template

This template is for app repos that already own their frontend and service
source trees.

It provides:

- `app_manifest.json` wiring the static site, live service, and vault service
- `services/live/*.json` and `services/vault/*.json` control-plane manifests
- `build-and-sync.sh`, `doctor.sh`, and the single first-release `release.sh`
- `.gitignore` entries for the expected service build outputs

It intentionally does not generate starter source under `frontend/`,
`services/live/app/`, or `services/vault/contract/`.

## What you need to replace

- point `app_manifest.json` at your real static-site dist directory if it is not
  already correct
- replace `build-and-sync.sh` with the commands that build your frontend, live
  bundle, and vault bytecode before calling `iroha soracloud service sync-manifests`
- add your own dev scripts if you want a one-command development entrypoint

## Release flow

```bash
./doctor.sh
TORII_URL=http://127.0.0.1:8080 ./release.sh
```

Root scripts require an absolute, non-symlinked `IROHA_BIN` plus its exact
`IROHA_BIN_SHA256`. Supply an operator-qualified CLI built from the same
revision as the workspace; there is no `PATH` lookup or source-build fallback.
