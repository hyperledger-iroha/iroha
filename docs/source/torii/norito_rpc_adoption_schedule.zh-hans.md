---
lang: zh-hans
direction: ltr
source: docs/source/torii/norito_rpc_adoption_schedule.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6065f2ea3bff221bf4df6ac3eef859c081f6e2bc503e4b6f59de440925d52827
source_last_modified: "2025-12-29T18:16:36.231226+00:00"
translation_last_reviewed: 2026-02-07
---

## Norito-RPC Cross-SDK Adoption Schedule (NRPC-4)

Status: Published 2026-03-22  
Owners: SDK Program Lead, Android Networking TL, Swift Lead, JS Lead, Python Maintainer  
Related roadmap item: NRPC-4 — Cross-SDK adoption schedule

> For the portal-facing summary that SDK teams share with external reviewers, see `docs/portal/docs/devportal/norito-rpc-adoption.md`. Keep the high-level messaging in sync with this canonical schedule.

### 1. Objectives
- Align Rust/CLI, Python, JavaScript, Android, and Swift transports on the binary Norito-RPC interface ahead of the AND4 production toggle.
- Ship a predictable phase plan so every SDK, CI lane, and observability surface is ready before JSON is demoted to fallback status.
- Keep fixtures, telemetry, and operator evidence consistent with the NRPC-2 rollout guardrails and the Android AND4 networking milestones.

### 2. Phased Timeline
| Phase | Window | Scope | Exit Criteria | Evidence |
|-------|--------|-------|---------------|----------|
| **P0 – Lab Parity** | Q2 2025 | Rust CLI + Python smoke suites exercise `/v2/pipeline` and `/v2/norito-rpc` in CI; JS helper passes unit tests; Android mock harness issues dual transport calls. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` wired into CI; `javascript/iroha_js/test/noritoRpcClient.test.js` green with `npm test`; Android mock harness (`java/iroha_android/src/test/java/.../NoritoRpcClientTests.java`) runs as part of `./gradlew test`. | CI logs + `status.md` entries; tracker rows NRPC-3A/NRPC-3 recorded as completed. |
| **P1 – SDK Preview** | Q3 2025 | Publish the shared fixture bundle (`fixtures/norito_rpc/lab/*.json`), record verification runs with `scripts/run_norito_rpc_fixtures.sh --sdk <label>` (drops logs + JSON under `artifacts/norito_rpc/`), and enable optional Norito transport flags in JS/Python/Swift samples. | Fixture bundle checked in with provenance manifest; README updates for JS (`javascript/iroha_js/README.md`) and Python (`python/iroha_python/README.md`) show opt-in usage; Swift preview API (NoritoBridge transport) available behind `IOS2` flag. | Fixture manifest + doc diffs recorded in `docs/source/torii/norito_rpc_tracker.md`; CI artifact links attached to `status.md`. |
| **P2 – Staging / AND4 Preview** | Q1 2026 | Staging Torii pools enable Norito by default; Android AND4 preview clients and Swift IOS2 parity suites hit the binary transport; telemetry dashboard `dashboards/grafana/torii_norito_rpc_observability.json` populated. | `docs/source/torii/norito_rpc_stage_reports.md` contains staging canary report; `scripts/telemetry/test_torii_norito_rpc_alerts.sh` green in CI; Android AND4 mock harness replay covers success/error paths; Swift `/v2/pipeline` adopters pass nightly parity run. | Stage report + Alertmanager test output attached to tracker entry NRPC-2R; AND4 readiness row in `roadmap.md` notes Norito coverage. |
| **P3 – Production GA** | Q4 2026 (aligned with AND4 production) | Norito transport becomes default for SDKs; JSON remains as brownout fallback; release pipeline archives Norito parity artefacts with every tag. | Release checklist contains Norito smoke output for Rust/JS/Python/Swift/Android; Alert thresholds for Norito vs JSON error rate SLOs enforced; `status.md` and release notes cite GA evidence. | `docs/source/runbooks/torii_norito_rpc_canary.md` updated with production execution; Alert exports + fixture checksums stored alongside signed manifests. |

### 3. SDK Deliverables & CI Coverage
| SDK / Surface | Key Actions | CI / Tests | Dependencies |
|---------------|-------------|------------|--------------|
| **Rust CLI & Integration Harness** | Extend `iroha_cli pipeline` smoke tests to run via the Norito transport once `cargo xtask norito-rpc-verify` lands; keep `integration_tests/tests/norito_streaming_roundtrip.rs` and friends targeting the binary transport as soon as Torii exposes it in devnet. | `cargo test -p integration_tests -- norito_streaming` (lab) and `cargo xtask norito-rpc-verify` (staging/GA) gated in CI; evidence uploaded under `artifacts/norito_rpc/`. | NRPC-2 rollout knobs, devnet Torii exposing `/v2/norito-rpc`. |
| **Python SDK** | Default `NoritoRpcClient` usage in the release smoke (`python/iroha_python/scripts/release_smoke.sh`), keep `run_norito_rpc_smoke.sh` as the canonical CI entrypoint, and document parity handling in `python/iroha_python/README.md`. | `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh` wired to `ci/python_norito_rpc_smoke.yml`; release smoke archives Norito JSON for `status.md`. | Fixtures from `fixtures/norito_rpc/`, NRPC-2 telemetry readiness, Torii dual-stack availability. |
| **JavaScript SDK** | Ship and stabilise `NoritoRpcClient` (already published), add governance/query wrappers that default to Norito when `toriiClientConfig.transport.preferred === "norito_rpc"`, and capture end-to-end samples in `javascript/iroha_js/recipes/`. | `npm test` (unit) + future `npm run test:norito-rpc` (dockerised integration) must pass before publish; provenance attaches Norito smoke output under `javascript/iroha_js/artifacts/`. | Shared fixture cadence, NRPC-2 telemetry, JS GA plan (JS-09/10). |
| **Android SDK (AND4)** | Keep Norito helpers in `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/NoritoRpcClient.java`, wire mock harness into AND4 preview lane, expose retry/backoff knobs through `ClientConfig`, and emit telemetry hooking into AND4 dashboards. | `./gradlew test` exercises Norito unit tests; AND4 preview lane replays mock harness flows nightly; chaos suite seeds failure cases once AND4 enters beta. | NRPC-2 staging pools, AND4 workshop outputs, telemetry dashboard readiness. |
| **Swift SDK (IOS2)** | Add Norito transport bindings via NoritoBridge (`IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`), reuse pipeline retry helpers, and add CI coverage inside `IrohaSwift/Tests/IrohaSwiftTests/NoritoRpcClientTests.swift`. Publish parity instructions in `docs/source/sdk/swift/quickstart.md`. | `xcodebuild test -scheme IrohaSwift-Package -destination "platform=iOS Simulator,name=iPhone 15"` run in Buildkite; parity metrics exported via `scripts/swift_status_export.py`. | `/v2/pipeline` adoption, NoritoBridge enhancements, localization staffing (IOS5) for docs. |
| **Docs & Portal** | Add binary transport samples + troubleshooting steps to `docs/portal/docs/devportal/torii-rpc-overview.md` and Try-It console once staging Norito gateway is reachable. | `docs/portal` preview build, netlify smoke for Try-It proxy, checksum enforcement via `build/checksums.sha256`. | DOCS-SORA workstream, NRPC-2 telemetry. |

### 4. Fixture & Automation Plan
1. **Fixture bundle (`NRPC-4F1`).** Generate canonical Norito-RPC request/response pairs for transactions, queries, and error paths using devnet data; store under `fixtures/norito_rpc/{lab,staging}/`. Provenance manifest mirrors the OpenAPI signing flow (SHA-256, BLAKE3, Ed25519 signature), and the schema table (`fixtures/norito_rpc/schema_hashes.json`) records the 16-byte hash for every DTO referenced in the spec.
2. **Regeneration cadence.** Regenerate fixtures every Wednesday at 17:00 UTC (or within 48 h of any data-model change). Ownership rotates weekly between the SDK Program Lead and the Android Networking TL and is tracked in the NRPC-4 row of `docs/source/torii/norito_rpc_tracker.md`. During the governance call, run `cargo xtask norito-rpc-fixtures` to refresh `.norito` payloads, the manifest, and `schema_hashes.json`, then sync the manifest into each SDK repo via the existing helper scripts.
3. **CI hooks.**  
   - Python: `run_norito_rpc_smoke.sh` (already live) promoted to required job once P0 completes.  
   - JS: extend `npm test` to call the mocked server harness (tracked under JS-10).  
   - Swift/Android: Buildkite / Gradle lanes capture Norito parity outputs and push them to the mobile parity dashboards.  
   - Rust: `cargo xtask norito-rpc-verify` fail-close gate executed in release CI and before publishing fixtures.
4. **Evidence archival.** Immediately after regeneration, run `scripts/run_norito_rpc_fixtures.sh --sdk <label> --note "<ticket>"` for each participating SDK. The wrapper executes `cargo xtask norito-rpc-verify`, writes the console log and JSON summary under `artifacts/norito_rpc/`, records the git SHA + dirty flag, **and** now archives the xtask verification report (`*-norito-rpc-xtask.json`) containing per-SDK manifest digests so stage reports and `status.md` entries can link to deterministic evidence.

### 5. Telemetry, Docs, and Reporting Hooks
- **Telemetry:** follow `docs/source/torii/norito_rpc_telemetry.md` for metric names (`torii_request_duration_seconds{scheme="norito_rpc"}`, `torii_norito_decode_failures_total`, etc.). SDK owners must ensure client logs redacted tokens and propagate `X-Iroha-Trace-Id` so staging comparisons stay deterministic.
- **Runbooks & Docs:** operator-facing steps live in `docs/source/torii/norito_rpc_rollout_plan.md` + `docs/source/runbooks/torii_norito_rpc_canary.md`. Developer-facing instructions are mirrored into the portal (`docs/portal/docs/devportal/torii-rpc-overview.md`) once Norito reaches P2. This schedule is now the canonical reference for cross-SDK deadlines.
- **Tracker:** NRPC-4 row added to `docs/source/torii/norito_rpc_tracker.md` with checklist items: fixture bundle, CI gating, SDK doc updates, and GA evidence. Owners update status weekly during the Torii platform sync.
- **Reporting cadence:**  
  - Weekly: share SDK parity deltas + outstanding blockers in Torii/SDK sync notes.  
  - Monthly: attach updated fixture checksums + telemetry screenshots to `status.md`.  
  - Release: include Norito smoke evidence + rollback plan excerpt in release notes.

### 6. Dependencies & Risks
| Risk | Impact | Mitigation / Owner |
|------|--------|--------------------|
| Android AND4 preview slips | Blocks production Norito timeline | Track in AND4 roadmap; SDK Program Lead ensures AND4 mock harness outputs feed this schedule before P2. |
| Swift NoritoBridge work under-resourced | iOS clients lag behind others | Swift lead pairs with Android/Python maintainers on fixture cadence; fallback plan keeps CLI/JS/Python as mandatory gate until IOS2 parity lands. |
| Fixture drift across SDKs | False negatives in CI, inconsistent telemetry | Shared regeneration script + governance review; `fixtures/norito_rpc` manifest requires two-signature approval (SDK + Torii). |
| Telemetry gaps (NRPC-2B) | Cannot prove SLOs during rollout | Observability liaison owns dashboard + alert validation before P2 exit; chaos script `scripts/telemetry/test_torii_norito_rpc_alerts.sh` must be green prior to stage promotion. |

With this schedule in place, NRPC-4’s acceptance criteria are met: every SDK has a published timeline, shared fixtures/game plan exist, and the dependencies back to NRPC-2/AND4 are documented so Norito-RPC can reach GA without ad-hoc coordination.

### 7. Governance & Rollout Gates
1. **NRPC-2R dual-stack readiness review (2025-06-19 Torii platform sync).** Platform Ops, NetOps, Observability, and the SDK Program Lead review the staging evidence bundle before `transport.norito_rpc.stage` can move beyond `lab`. The review requires:
   - A fresh entry in `docs/source/torii/norito_rpc_stage_reports.md` and the tracker (`docs/source/torii/norito_rpc_tracker.md`) that links to the canary log, participating clusters, and rollback plan.
   - Successful execution of `scripts/telemetry/test_torii_norito_rpc_alerts.sh`, with the JSON/NDJSON output archived under `artifacts/norito_rpc/telemetry/<stamp>/`.
   - Updated fixture/schema hashes in `fixtures/norito_rpc/schema_hashes.json` that match the SDK builds attached to the report.
2. **Stage toggles for `transport.norito_rpc.stage`.** After NRPC-2R, Platform Ops promotes a subset of Torii pools to `staging_canary`, records the timestamp + cluster list inside the tracker and `status.md`, and keeps the override notes in `docs/source/torii/norito_rpc_rollout_plan.md`. Promotion to `production` follows the same pattern once the AND4 milestone closes and GA telemetry stays green for the burn-in window.
3. **Telemetry guardrails.** SRE must keep `dashboards/grafana/torii_norito_rpc_observability.json` and the alert definitions in `docs/source/torii/norito_rpc_telemetry.md` current. Stage promotions require attaching the latest screenshots/JSON exports to the tracker row so governance reviewers can confirm latency/error budgets meet the NRPC-2B thresholds.

### 8. Stage Evidence & Reporting Checklist
- **Tracker hygiene.** Every stage transition adds a dated row to `docs/source/torii/norito_rpc_tracker.md` with the stage (`lab`, `staging_canary`, `production`), DRIs, telemetry artefact links, and references to the relevant `status.md` entry.
- **Release bundles.** Bundle `cargo xtask norito-rpc-verify`, `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `javascript/iroha_js/scripts/run-norito-rpc-e2e.mjs`, Android AND4 replay logs, and the Swift parity suite under `artifacts/norito_rpc/<sdk>/<stamp>/` with SHA256 digests noted in `status.md`.
- **Portal + operator comms.** Update this file and `docs/portal/docs/devportal/norito-rpc-adoption.md` in the same PR when stages change, and include links to the announcement/FAQ so Support teams can reuse the messaging.
- **Telemetry exports.** Store AlertManager self-test bundles, Grafana exports, and the executed `docs/source/runbooks/torii_norito_rpc_canary.md` log in `artifacts/norito_rpc/telemetry/<stamp>/` and link them from the tracker to satisfy NRPC-2 gating.
