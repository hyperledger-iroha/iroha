---
lang: my
direction: ltr
source: docs/source/torii/norito_rpc_rollout_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 07af3011a1deedd7f6080b69d073ec6a683d9d2f0efbb96aefe59511f16c929f
source_last_modified: "2025-12-29T18:16:36.232983+00:00"
translation_last_reviewed: 2026-02-07
---

## Norito-RPC Rollout Plan (NRPC-2)

Status: Drafted 2026-03-21  
Owners: Torii Platform TL, NetOps Liaison, Observability Liaison, Platform Ops  
Related roadmap item: NRPC-2 — Torii server rollout plan

### Scheduled Reviews
- **Dual-stack readiness review** — 2025-06-19 15:00 UTC (Torii Platform sync). Agenda: confirm `docs/source/torii/nrpc_spec.md` is frozen, walk through `transport.norito_rpc.*` config knobs, validate ingress/mTLS plans, and record action items for SDK/Android owners before staging canary. Minutes land in `docs/source/torii/norito_rpc_sync_notes.md` and tracker entry `NRPC-2R`.

### 1. Objectives
- Enable the Norito-RPC transport defined in `docs/source/torii/norito_rpc.md` without disrupting existing JSON callers.
- Ship ingress/auth configuration, telemetry, and rollback guards that keep Torii deterministic across environments.
- Provide an operator-facing checklist that stages adoption through lab → staging → canary → GA with rehearsed fallback.

### 2. Environment & Configuration Baseline
| Environment | Required State | Config knobs | Notes |
|-------------|----------------|--------------|-------|
| Lab (CI + devnet) | Norito-RPC enabled by default; JSON kept for parity tests. | `iroha_config::Client::transport.norito_rpc.enabled = true` (defaulted in `crates/iroha_config/src/parameters/defaults.rs`). | CI runs dual-transport smoke tests (`python/iroha_python/scripts/run_norito_rpc_smoke.sh:1`). |
| Staging | Norito enabled on dedicated pool; mTLS enforced for partner ingress. | Add `transport.norito_rpc.require_mtls` and `transport.norito_rpc.allowed_clients` ACL during rollout. | Verify reverse proxies propagate `Content-Type: application/x-norito`. |
| Production | Feature behind `transport.norito_rpc.stage` (`disabled` \| `canary` \| `ga`). | Canary stage restricts Norito to allowlist tokens; GA removes restriction. | Stage promotes only after telemetry and alerts reach green baselines. |

The new `transport.norito_rpc` block sits under `client_api` defaults, mirroring existing handshake flags (`crates/iroha_config/src/client_api.rs:904`). Platform Ops owns schema evolution and documentation in `docs/source/config/client_api.md` once knobs land.

### 3. Ingress & Authentication Changes (NRPC-2A)
1. **Dual-stack listeners**  
   - Keep a single Torii listener but ensure Envoy/Nginx front-ends route `Content-Type: application/x-norito` traffic without compression rewrites.  
   - Update the shared ingress cookbook with a deterministic header allowlist and explicit `httpFilters` for gRPC to avoid content-type coercion.
2. **mTLS policy**  
   - Stage/production canary pools require client certificates signed by the operator CA; fallback to bearer tokens is kept for devnet.  
   - Document the SAN expectations and rotation cadence in `defaults/torii_ingress_mtls.yaml` (new) so integrators can mirror the policy.
3. **Rate limiting & ACL alignment**  
   - `ConnScheme::NoritoRpc` is already tagged by the pre-auth gate (`crates/iroha_torii/src/lib.rs:683`). Add per-scheme limit buckets so Norito can be throttled independently.  
   - Extend `network_acl` validation to accept `scheme=norito_rpc` selectors, ensuring deny/allow lists can treat Norito separately.
4. **Error surface**  
   - Update operator docs to include schema-mismatch and checksum errors raised by the Norito header validator (see Section 13 of `docs/source/torii/norito_rpc.md`).  
- Ensure 4xx/5xx responses propagate through proxies with deterministic retry hints (`Retry-After`, `X-Iroha-Error-Code`). Torii now stamps `Retry-After: 300` on `norito_rpc_disabled`/`norito_rpc_canary_denied` envelopes so SDKs can record the downgrade window before checking the stage again.
5. **Scheme-aware pre-auth caps**  
   - `torii.preauth_scheme_limits` (threaded through `crates/iroha_config/src/parameters/{user,actual}.rs` and enforced by `crates/iroha_torii/src/limits.rs`) now lets operators bound `norito_rpc` sessions independently from HTTP/WS traffic.  
   - Default keeps the field empty (inherit global caps); production staging should set a conservative `norito_rpc` bucket before flipping the stage flag so runaway binary clients cannot starve JSON callers.

Deliverables for NRPC-2A: updated ingress cookbook, config knobs, and automated validation that rejects requests when `Content-Type` is stripped (negative regression tests in `crates/iroha_torii/tests/norito_ingress.rs`, new).

### 4. Telemetry & Alerting Updates (NRPC-2B)
1. **Metrics**  
   - Continue using `torii_active_connections_total{scheme="norito_rpc"}` exposed via `Telemetry::inc_torii_active_conn` (`crates/iroha_core/src/telemetry.rs:3349`).  
   - The HTTP middleware now records `torii_request_duration_seconds{scheme}` and `torii_request_failures_total{scheme,code}` so Norito traffic has dedicated latency/error SLIs (`crates/iroha_torii/src/lib.rs:1179`, `crates/iroha_core/src/telemetry.rs:3827`).  
   - Wire Norito decode errors (`Unsupported feature: layout flag`, `checksum mismatch`) into structured logs and increment `torii_norito_decode_failures_total`.
2. **Dashboards**  
   - Publish a new Grafana dashboard `dashboards/grafana/torii_norito_rpc_observability.json` with panels for request volume, latency p50/p95, pre-auth rejects, and error codes.  
   - Include comparison plots with JSON traffic to ensure parity during rollout.
3. **Alerting**  
   - Add Alertmanager rules in `dashboards/alerts/torii_norito_rpc_rules.yml` that fire when error rate >1% over 5 minutes or when canary connections drop below baseline.  
   - Chaos scripts should replay malformed Norito envelopes weekly; capture evidence in `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.

NRPC-2B exits when dashboards show live data in staging, alert runbooks exist, and chaos drills populate `status.md` with the latest rehearsal timestamp.

### 5. Rollout Checklist & Phases (NRPC-2C)
| Phase | Exit Criteria | Verification |
|-------|---------------|--------------|
| **P0 — Lab validation** | Config knobs merged with defaults; dual-transport CI passing; ingress cookbook updated. | `cargo test -p iroha_torii -- norito_ingress` and `python/iroha_python/scripts/run_norito_rpc_smoke.sh:1`. |
| **P1 — Staging canary** | Norito enabled for internal tokens; Grafana dashboard populated; alerts quiet for 72h. | Manual canary report archived in `docs/source/torii/norito_rpc_stage_reports.md`. |
| **P2 — Production canary** | `transport.norito_rpc.stage=canary`; allowlist tokens cover at least two operators; rollback rehearsed. | Deploy playbook recorded in `docs/source/runbooks/torii_norito_rpc_canary.md`. |
| **P3 — GA** | Stage flag flipped to `ga`; JSON + Norito parity fixtures green; ops sign-off logged. | Run `cargo xtask norito-rpc-verify` (new) to compare transports against the canonical fixtures in `fixtures/norito_rpc/transaction_fixtures.manifest.json`. |

Detailed operator steps for P2 live in
[`docs/source/runbooks/torii_norito_rpc_canary.md`](../runbooks/torii_norito_rpc_canary.md),
and every canary/GA window must update the evidence log at
[`docs/source/torii/norito_rpc_stage_reports.md`](norito_rpc_stage_reports.md)
before the change ticket can close.

Each phase requires explicit rollback instructions: disable the Norito stage flag (`transport.norito_rpc.stage=disabled`), invalidate Envoy route configuration, and clear canary allowlist. Platform Ops maintains the playbook.

### 6. Brownout & Dual-stack Guardrails

The rollout must remain reversible without starving JSON callers. This section
defines the brownout signals, the config switches that protect `/v1/pipeline`,
and the validation steps required before closing an incident.

#### 6.1 Detection & Entry Criteria
- Monitor `torii_request_duration_seconds{scheme="norito_rpc"}` and
  `torii_request_failures_total{scheme="norito_rpc",code}` alongside
  `torii_norito_decode_failures_total` and `torii_active_connections_total`
  on the Norito dashboard (`dashboards/grafana/torii_norito_rpc_observability.json`)
  and telemetry spec (`docs/source/torii/norito_rpc_telemetry.md`). Brownout
  consideration begins when error rate exceeds 1 % for two consecutive 5‑minute
  windows or decode failures spike outside scheduled chaos drills.
- Alertmanager rules in `dashboards/alerts/torii_norito_rpc_rules.yml`, validated
  via `scripts/telemetry/test_torii_norito_rpc_alerts.sh`, must remain green.
  An alert firing automatically opens the brownout checklist.
- Record the trigger, timestamps, and Grafana export hashes in
  `docs/source/torii/norito_rpc_stage_reports.md` so every brownout leaves an
  evidence trail.

#### 6.2 Brownout Procedure
1. **Clamp transport stage:** change `transport.norito_rpc.stage` to `canary` or
   `disabled` in the client API config (`docs/source/config/client_api.md`), apply
   the patch through admission, and call out the change in
   `docs/source/runbooks/torii_norito_rpc_canary.md`. This immediately forces
   non-allowlisted clients back to JSON while keeping canary probes alive.
2. **Throttle scheme-specific capacity:** use
   `torii.preauth_scheme_limits.norito_rpc` (see
   `crates/iroha_config/src/parameters/user.rs:6057` and
   `crates/iroha_config/src/parameters/actual.rs:1805`) to cap the number of
   active Norito connections so JSON callers keep their global budget even if
   binary clients misbehave.
3. **Prune canary tokens:** remove all but the diagnostic allowlist entries so
   only the official smoke harness can reach the Norito routes during the
   brownout, and document the revocation in
   `docs/source/torii/norito_rpc_stage_reports.md`.
4. **Publish telemetry snapshot:** capture the Norito dashboard, the JSON
   comparison panels, and the alert-test output
   (`scripts/telemetry/test_torii_norito_rpc_alerts.sh --env <env>`) and attach
   them to the incident ticket together with the config hash.
5. **Coordinate rollback:** once the culprit is mitigated, reapply the desired
   stage/limits following the canary runbook and update the stage report with a
   close-out summary.

#### 6.3 JSON `/v1/pipeline` Fallback Validation
- `/v1/pipeline` stays live even when Norito is degraded (coexistence is defined
  in `docs/source/torii/norito_rpc.md:170`), so every brownout must prove the
  JSON path is healthy:
  1. Issue a Norito probe (allowlisted token) using the Python helper  
     `python/iroha_python/scripts/run_norito_rpc_smoke.sh` to confirm the canary
     transport still succeeds while non-allowlisted calls are rejected with
     the expected `norito_rpc_canary_denied`/`norito_rpc_disabled` codes.
  2. Exercise the JSON pipeline with a standard submission/status pair, e.g.:
     ```bash
     curl -sS -X POST "$TORII_URL/v1/pipeline/transactions" \
       -H 'Content-Type: application/json' \
       -d @<signed_tx.json>
     curl -sS "$TORII_URL/v1/pipeline/transactions/status?hash=<tx_hash>"
     ```
     or the equivalent SDK helper. Attach the responses to the incident log.
- Re-run the JSON smoke after Norito is re-enabled to confirm both transports
  share the same queue depth and latency budgets before closing the brownout.

### 7. Communication & Stakeholder Actions
- **Platform Ops**: circulate rollout window and rollback plan 48 hours prior to each phase; capture outcomes in `status.md`.  
- **NetOps**: confirm ingress + mTLS artefacts replicated across regions before promoting from staging.  
- **Observability**: sign off on dashboard screenshots and alert test outputs.  
- **SDK Program**: notify partners when Norito is available in staging so SDK parity tests can switch to binary transport.  
- **Docs/DevRel**: update the developer portal once GA completes, embedding CLI examples that set Norito headers.

### 8. Ownership & Tracker Alignment
- NRPC-2A completes when ingress cookbook, config knobs, and tests merge (update `docs/source/torii/norito_rpc_tracker.md` status to 🈴).  
- NRPC-2B closes once the telemetry spec, alert rules, and validation scripts land (see `docs/source/torii/norito_rpc_telemetry.md` and associated Prometheus tests).  
- NRPC-2C completes with the signed rollout checklist and runbooks merged; add the final artefacts to the tracker notes.  
- Weekly Torii platform sync reviews checklist progress; deviations escalate via the SNNet governance process described in `docs/source/torii/norito_rpc_sync_notes.md`.

### 9. Appendix — Rollback Quick Reference
1. Flip `transport.norito_rpc.stage` to `disabled` via `iroha_config` Norito patch + admission reload.  
2. Remove Norito-specific routes from Envoy/Nginx and reload proxies.  
3. Drain existing Norito connections by revoking canary tokens and watching `torii_active_connections_total{scheme="norito_rpc"}` fall to zero.  
4. Re-run JSON pipeline smoke tests to confirm service availability.  
5. File a post-mortem entry in `docs/source/postmortems/norito_rpc_rollback.md` within 24 hours.

With this plan, Torii operators can ship Norito-RPC deterministically while preserving a clear escape hatch and observability story, fulfilling the NRPC-2 roadmap milestone.
