---
lang: ur
direction: rtl
source: docs/source/torii/norito_rpc_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f7561ec51c6b3cb8c86155ce5713ea53250f92d383a5e0ce96593fb2dacd839
source_last_modified: "2026-01-03T18:07:58.835510+00:00"
translation_last_reviewed: 2026-01-30
---

## Norito-RPC Action Tracker

| ID | Description | Owner(s) | Status | Target | Notes |
|----|-------------|----------|--------|--------|-------|
| NRPC-2A | Draft Torii ingress/auth rollout plan covering dual-stack, mTLS, and fallback guards | Torii Platform TL, NetOps | 🈴 Completed | 2026-03-21 | Section 3 of `docs/source/torii/norito_rpc_rollout_plan.md` documents ingress policy, mTLS requirements, rate-limit alignment, and error handling. |
| NRPC-2B | Define telemetry + alert updates (Grafana, Alertmanager, chaos scripts) | Observability liaison | 🈴 Completed | 2026-03-19 | Telemetry spec + alert suite published (`docs/source/torii/norito_rpc_telemetry.md`, `dashboards/grafana/torii_norito_rpc_observability.json`, `dashboards/alerts/torii_norito_rpc_rules.yml`, `scripts/telemetry/test_torii_norito_rpc_alerts.sh`). |
| NRPC-2C | Produce rollout checklist (stage, canary, GA, rollback) | Platform Ops | 🈴 Completed | 2026-03-21 | Sections 5 and 8 of `docs/source/torii/norito_rpc_rollout_plan.md` publish the staged checklist plus rollback quick reference. |
| NRPC-2R | Dual-stack readiness review (freeze spec + rollout knobs) | Torii Platform TL, SDK Program Lead, Android Networking TL | 🈴 Completed | 2025-06-19 | Review held 2025-06-19; spec + fixtures frozen (`nrpc_spec.md` SHA-256 `0bb9d2c225b5485fd0b7c6ef28a3ecea663afea76b09a248701d0b50c25982b1`, `fixtures/norito_rpc/schema_hashes.json` SHA-256 `343f4c7991e6bcfbda894b3b2422a0241def279fc79db7563da618d31763ba1c`). Canary rules: `transport.norito_rpc.stage=canary` requires mTLS + `allowed_clients`, JSON stays available for rollback, telemetry gates via `dashboards/grafana/torii_norito_rpc_observability.json`. Minutes in `docs/source/torii/norito_rpc_sync_notes.md`. |
| NRPC-3A | JavaScript SDK helper + tests for Norito-RPC requests | JS SDK Lead | 🈴 Completed | 2026-04-15 | `NoritoRpcClient` + `NoritoRpcError` now ship in the SDK with TypeScript definitions, documentation, and unit tests covering headers, params, retries, and timeout behaviour (`javascript/iroha_js/src/noritoRpcClient.js`,`javascript/iroha_js/test/noritoRpcClient.test.js`,`javascript/iroha_js/index.d.ts`,`javascript/iroha_js/README.md:342`). |
| NRPC-3B | Swift SDK Norito transport helper | Swift SDK Lead | 🈴 Completed | 2026-04-17 | `NoritoRpcClient` now ships under `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift` with regression coverage (`IrohaSwift/Tests/IrohaSwiftTests/NoritoRpcClientTests.swift`) and docs/README updates so Swift matches the JS helper for NRPC adoption. |
| NRPC-3C | Cross-SDK fixture cadence agreement | SDK Program Lead | 🈴 Completed | 2026-04-19 | Weekly Wed 17:00 UTC rotation + procedure captured in `docs/source/torii/norito_rpc_fixture_cadence.md`, pointing owners at `scripts/run_norito_rpc_fixtures.sh`, artefact paths (`artifacts/norito_rpc/<stamp>/`), and status logging expectations. |
| NRPC-4 | Publish cross-SDK adoption schedule & checklist | SDK Program Lead / Swift & JS Leads | 🈴 Completed | 2026-03-22 | `docs/source/torii/norito_rpc_adoption_schedule.md` captures the phased timeline, SDK deliverables, fixture plan, and telemetry/reporting hooks needed to align NRPC GA with AND4. |
| DOC-NRPC | Developer portal update and Try-It console samples | Docs/DevRel | 🈴 Completed | 2026-03-25 | Portal docs now cover Norito payloads end to end (`docs/portal/docs/devportal/try-it.md`,`docs/portal/docs/norito/try-it-console.md`), proxy tagging (`TRYIT_PROXY_CLIENT_ID` → `X-TryIt-Client`) is wired and tested in `tryit-proxy-lib.mjs` tests, and the browser “Try it” path exercises `application/x-norito` fixtures with the signed manifest guard still enforced. |
| OPS-NRPC | Operator FAQ update (status.md, release notes) | DevRel, Ops | 🈴 Completed | 2026-03-29 | Operator FAQ now carries the ready-to-drop release note text + publishing steps in §5 (`docs/source/runbooks/torii_norito_rpc_faq.md:97`–`116`), so on-call owners can announce the rollout without drafting ad-hoc copy. |
| QA-NRPC | Extend smoke tests for Norito vs JSON parity | QA Guild | 🈴 Completed | 2026-04-22 | `cargo xtask norito-rpc-verify` now exercises the alias endpoints using both JSON and Norito payloads to flag regressions before rollout.【xtask/src/main.rs:492】【xtask/src/main.rs:3517】 |
| NRPC-4F1 | Fixture cadence & evidence automation | SDK Program Lead / Android Networking TL | 🈴 Completed | Weekly (Wed 17:00 UTC) | Cadence wrapper now emits rotation summaries automatically (`--auto-report` → `artifacts/norito_rpc/rotation_status.{json,md}` with 7‑day freshness) alongside per-run logs/JSON/xtask output. Owners still alternate weekly rotations; artefact paths remain under `artifacts/norito_rpc/` for `status.md` and governance packets. |

### Update cadence
- Reviewed weekly in Torii platform sync.
- Tracker owners update status ahead of the Tuesday dry run meeting.
