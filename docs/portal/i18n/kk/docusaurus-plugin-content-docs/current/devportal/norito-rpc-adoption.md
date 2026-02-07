---
id: norito-rpc-adoption
lang: kk
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
---

> Canonical planning notes live in `docs/source/torii/norito_rpc_adoption_schedule.md`.  
> This portal copy distils the rollout expectations for SDK authors, operators, and reviewers.

## Objectives

- Align every SDK (Rust CLI, Python, JavaScript, Swift, Android) on the binary Norito-RPC transport ahead of the AND4 production toggle.
- Keep phase gates, evidence bundles, and telemetry hooks deterministic so governance can audit the rollout.
- Make it trivial to capture fixture and canary evidence with the shared helpers that roadmap NRPC-4 calls out.

## Phase timeline

| Phase | Window | Scope | Exit Criteria |
|-------|--------|-------|---------------|
| **P0 – Lab parity** | Q2 2025 | Rust CLI + Python smoke suites run `/v1/norito-rpc` in CI, JS helper passes unit tests, Android mock harness exercises dual transports. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` and `javascript/iroha_js/test/noritoRpcClient.test.js` green in CI; Android harness wired into `./gradlew test`. |
| **P1 – SDK preview** | Q3 2025 | Shared fixture bundle checked in, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` records logs + JSON in `artifacts/norito_rpc/`, optional Norito transport flags exposed in SDK samples. | Fixture manifest signed, README updates show opt-in usage, Swift preview API available behind the IOS2 flag. |
| **P2 – Staging / AND4 preview** | Q1 2026 | Staging Torii pools prefer Norito, Android AND4 preview clients and Swift IOS2 parity suites default to the binary transport, telemetry dashboard `dashboards/grafana/torii_norito_rpc_observability.json` populated. | `docs/source/torii/norito_rpc_stage_reports.md` captures the canary, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` passes, Android mock harness replay captures success/error cases. |
| **P3 – Production GA** | Q4 2026 | Norito becomes the default transport for all SDKs; JSON remains a brownout fallback. Release jobs archive parity artefacts with every tag. | Release checklist bundles Norito smoke output for Rust/JS/Python/Swift/Android; Alert thresholds for Norito vs JSON error-rate SLOs enforced; `status.md` and release notes cite GA evidence. |

## SDK deliverables & CI hooks

- **Rust CLI & integration harness** – extend `iroha_cli pipeline` smoke tests to force the Norito transport once `cargo xtask norito-rpc-verify` lands. Guard with `cargo test -p integration_tests -- norito_streaming` (lab) and `cargo xtask norito-rpc-verify` (staging/GA), storing artefacts under `artifacts/norito_rpc/`.
- **Python SDK** – default the release smoke (`python/iroha_python/scripts/release_smoke.sh`) to Norito RPC, keep `run_norito_rpc_smoke.sh` as the CI entrypoint, and document parity handling in `python/iroha_python/README.md`. CI target: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **JavaScript SDK** – stabilise `NoritoRpcClient`, let governance/query helpers default to Norito when `toriiClientConfig.transport.preferred === "norito_rpc"`, and capture end-to-end samples in `javascript/iroha_js/recipes/`. CI must run `npm test` plus the dockerised `npm run test:norito-rpc` job before publish; provenance uploads Norito smoke logs under `javascript/iroha_js/artifacts/`.
- **Swift SDK** – wire the Norito bridge transport behind the IOS2 flag, mirror the fixture cadence, and ensure the Connect/Norito parity suite runs inside the Buildkite lanes referenced in `docs/source/sdk/swift/index.md`.
- **Android SDK** – AND4 preview clients and the mock Torii harness adopt Norito, with retry/backoff telemetry documented in `docs/source/sdk/android/networking.md`. The harness shares fixtures with other SDKs via `scripts/run_norito_rpc_fixtures.sh --sdk android`.

## Evidence & automation

- `scripts/run_norito_rpc_fixtures.sh` wraps `cargo xtask norito-rpc-verify`, captures stdout/stderr, and emits `fixtures.<sdk>.summary.json` so SDK owners have a deterministic artefact to attach to `status.md`. Use `--sdk <label>` and `--out artifacts/norito_rpc/<stamp>/` to keep CI bundles tidy.
- `cargo xtask norito-rpc-verify` enforces schema hash parity (`fixtures/norito_rpc/schema_hashes.json`) and fails if Torii returns `X-Iroha-Error-Code: schema_mismatch`. Pair every failure with a JSON fallback capture for debugging.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` and `dashboards/grafana/torii_norito_rpc_observability.json` define the alert contracts for NRPC-2. Run the script after every dashboard edit and store the `promtool` output in the canary bundle.
- `docs/source/runbooks/torii_norito_rpc_canary.md` describes the staging and production drills; update it whenever fixture hashes or alert gates change.

## Reviewer checklist

Before ticking an NRPC-4 milestone, confirm:

1. Latest fixture bundle hashes match `fixtures/norito_rpc/schema_hashes.json` and the corresponding CI artefact recorded under `artifacts/norito_rpc/<stamp>/`.
2. SDK README / portal docs describe how to force JSON fallback and cite the Norito transport default.
3. Telemetry dashboards show dual-stack error-rate panels with alert links, and the Alertmanager dry run (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) is attached to the tracker.
4. The adoption schedule here matches the tracker entry (`docs/source/torii/norito_rpc_tracker.md`) and the roadmap (NRPC-4) references the same evidence bundle.

Staying disciplined on the schedule keeps cross-SDK behaviour predictable and lets governance audit Norito-RPC adoption without bespoke requests.
