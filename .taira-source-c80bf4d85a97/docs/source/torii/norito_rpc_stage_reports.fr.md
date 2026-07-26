---
lang: fr
direction: ltr
source: docs/source/torii/norito_rpc_stage_reports.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20cd29e6dd0bb03ff2d2e628deb3097dd72db57b88223aee28ae1c04df0e7126
source_last_modified: "2026-01-03T18:07:58.907776+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Norito-RPC Stage Reports

This log captures every Norito-RPC rollout step (staging canary, production
canary, GA). Populate the tables below whenever the canary runbook executes.
Attach artefacts under `artifacts/norito_rpc/<YYYYMMDD>/` and reference their
hashes in the “Evidence” column.

## Report Legend

| Field | Description |
|-------|-------------|
| Date (UTC) | ISO timestamp when the stage window started. |
| Stage | `staging-canary`, `production-canary`, or `ga`. |
| Config digest | SHA-256 of the Norito config patch applied through admission. |
| Allowlist hash | SHA-256 of the canary token manifest (`allowed_clients`). |
| Smoke bundle | Hash of the `run_norito_rpc_smoke.sh` output TAR/ZIP. |
| Grafana export | Hash of the dashboard JSON export capturing telemetry. |
| Alert drill ID | Identifier produced by `scripts/telemetry/test_torii_norito_rpc_alerts.sh`. |
| Notes | Free-form observations, incident links, or follow-ups. |

### Capturing hashes

Use deterministic inputs so every operator records identical digests:

```bash
# Config patch (Norito JSON/TOML)
shasum -a 256 artifacts/norito_rpc/20260405/config_patch.json

# Allowlist manifest
shasum -a 256 artifacts/norito_rpc/20260405/allowlist.json

# Smoke bundle (tarball produced by run_norito_rpc_smoke.sh --bundle)
shasum -a 256 artifacts/norito_rpc/20260405/smoke_bundle.tar.gz

# Grafana export
shasum -a 256 artifacts/norito_rpc/20260405/grafana_torii_norito_rpc.json
```

Record the resulting hex strings in the table along with the alert drill identifier
printed by `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.

### Recording alert drill IDs

Run `scripts/telemetry/test_torii_norito_rpc_alerts.sh` after each canary window. The script
executes the Prometheus rule tests (locally or via Docker) and prints a deterministic line such
as:

```text
NORITO_RPC_ALERT_DRILL_ID=nrpc-alert-20260411T181500Z-a1b2c3d4e5f6-7f9b2a83d5c1
```

Copy the entire value (everything after the `=`) into the “Alert drill ID” column so the runbook
records which alert bundle produced the evidence.

## Staging Canary Runs

| Date (UTC) | Stage | Config digest | Allowlist hash | Smoke bundle | Grafana export | Alert drill ID | Notes |
|------------|-------|---------------|----------------|--------------|----------------|----------------|-------|
| 2026-04-11T18:00:00Z | staging-canary | recorded in `artifacts/norito_rpc/rotation_status.json` | recorded in `artifacts/norito_rpc/rotation_status.json` | workflow bundle from `.github/workflows/torii-mock-harness.yml` | `dashboards/grafana/torii_norito_rpc_observability.json` export | nrpc-alert-20251112T074326Z-39b361f58e5e-b9c4f7e37135 | Evidence captured via torii-mock-harness CI + `scripts/run_norito_rpc_fixtures.sh --auto-report`; replace with new hashes on the next canary window. |

## Production Canary Runs

| Date (UTC) | Stage | Config digest | Allowlist hash | Smoke bundle | Grafana export | Alert drill ID | Notes |
|------------|-------|---------------|----------------|--------------|----------------|----------------|-------|
| _TBD_ | production-canary | | | | | | |

## GA Promotions

| Date (UTC) | Stage | Config digest | Allowlist hash | Smoke bundle | Grafana export | Alert drill ID | Notes |
|------------|-------|---------------|----------------|--------------|----------------|----------------|-------|
| _TBD_ | ga | | N/A | | | N/A | |

Remember to update `status.md` after each entry so stakeholders can track
progress without consulting this log directly.
