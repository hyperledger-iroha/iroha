---
lang: mn
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 769d285b34e9eb49a12f80928078358527441ebc86a1d7250b88b4f9680e1c70
source_last_modified: "2025-12-29T18:16:36.030036+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Norito-RPC Operator FAQ

This FAQ distills the rollout/rollback knobs, telemetry, and evidence artefacts
referenced in roadmap items **NRPC-2** and **NRPC-4** so operators have a single
page to consult during canaries, brownouts, or incident drills. Treat it as the
front door for on-call handoffs; detailed procedures still live in
`docs/source/torii/norito_rpc_rollout_plan.md` and
`docs/source/runbooks/torii_norito_rpc_canary.md`.

## 1. Configuration knobs

| Path | Purpose | Allowed values / notes |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | Hard on/off switch for the Norito transport. | `true` keeps the HTTP handlers registered; `false` disables them regardless of stage. |
| `torii.transport.norito_rpc.require_mtls` | Enforce mutual TLS for Norito endpoints. | Default `true`. Turn off only in isolated staging pools. |
| `torii.transport.norito_rpc.allowed_clients` | Whitelist of service accounts / API tokens that may use Norito. | Supply CIDR blocks, token hashes, or OIDC client IDs depending on your deployment. |
| `torii.transport.norito_rpc.stage` | Rollout stage advertised to SDKs. | `disabled` (reject Norito, force JSON), `canary` (allow only the allowlist, trigger enhanced telemetry), `ga` (default-on for every authenticated client). |
| `torii.preauth_scheme_limits.norito_rpc` | Per-scheme concurrency + burst budget. | Mirror the keys used for the HTTP/WS throttles (e.g., `max_in_flight`, `rate_per_sec`). Raising the cap without updating Alertmanager defeats the rollout guard. |
| `transport.norito_rpc.*` in `docs/source/config/client_api.md` | Client-facing overrides (CLI / SDK discovery). | Use `cargo xtask client-api-config diff` to inspect pending changes before pushing them to Torii. |

**Recommended brownout flow**

1. Set `torii.transport.norito_rpc.stage=disabled`.
2. Keep `enabled=true` so probes/alert tests still exercise the handlers.
3. Flip `torii.preauth_scheme_limits.norito_rpc.max_in_flight` to zero if you
   need an immediate stop (for example, while waiting for config propagation).
4. Update the operator log and attach the new config digest to the stage report.

## 2. Operational checklists

- **Canary / staging runs** — follow `docs/source/runbooks/torii_norito_rpc_canary.md`.
  That runbook references the same config keys above and lists the evidence
  artefacts captured by `scripts/run_norito_rpc_smoke.sh` +
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.
- **Production promotion** — execute the stage report template in
  `docs/source/torii/norito_rpc_stage_reports.md`. Record the config hash,
  allowlist hash, smoke bundle digest, Grafana export hash, and the alert drill
  identifier.
- **Rollback** — switch `stage` back to `disabled`, keep the allowlist in place,
  and document the switch in the stage report + incident log. When the root
  cause is fixed, re-run the canary checklist before setting `stage=ga`.

## 3. Telemetry & alerts

| Asset | Location | Notes |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | Tracks request rate, error codes, payload sizes, decode failures, and adoption %. |
| Alerts | `dashboards/alerts/torii_norito_rpc_rules.yml` | `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures`, and `NoritoRpcFallbackSpike` gates. |
| Chaos script | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | Fails CI when alert expressions drift. Run it after every config change. |
| Smoke tests | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | Include their logs in the evidence bundle for each promotion. |

Dashboards must be exported and attached to the release ticket (`make
docs-portal-dashboards` in CI) so that on-call engineers can replay the metrics
without access to production Grafana.

## 4. Frequently asked questions

**How do I allow a new SDK during canary?**  
Add the service account/token to `torii.transport.norito_rpc.allowed_clients`,
reload Torii, and record the change in `docs/source/torii/norito_rpc_tracker.md`
under NRPC-2R. The SDK owner must also capture a fixture run via
`scripts/run_norito_rpc_fixtures.sh --sdk <label>`.

**What happens if Norito decoding fails mid-rollout?**  
Leave `stage=canary`, keep `enabled=true`, and triage the failures via
`torii_norito_decode_failures_total`. SDK owners can fall back to JSON by
omitting `Accept: application/x-norito`; Torii will continue serving JSON until
the stage switches back to `ga`.

**How do I prove the gateway is serving the right manifest?**  
Run `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host
<gateway-host>` so the probe records the `Sora-Proof` headers alongside the
Norito config digest. Attach the JSON output to the stage report.

**Where should I capture redaction overrides?**  
Document every temporary override in the stage report’s `Notes` column and log
the Norito config patch under change control. Overrides expire automatically in
the config file; the FAQ entry ensures on-call engineers remember to clean up
after incidents.

For any question not covered here, escalate via the channels listed in the
canary runbook (`docs/source/runbooks/torii_norito_rpc_canary.md`).

## 5. Release note snippet (OPS-NRPC follow-up)

Roadmap item **OPS-NRPC** requires a ready-to-drop release note so operators
announce the Norito-RPC rollout consistently. Copy the block below into the next
release post (replace the bracketed fields) and attach the evidence bundle
described underneath.

> **Torii Norito-RPC transport** — Norito envelopes are now served alongside the
> JSON API. The `torii.transport.norito_rpc.stage` flag ships set to
> **[stage: disabled/canary/ga]** and follows the staged rollout checklist in
> `docs/source/torii/norito_rpc_rollout_plan.md`. Operators can opt out temporarily
> by setting `torii.transport.norito_rpc.stage=disabled` while leaving
> `torii.transport.norito_rpc.enabled=true`; SDKs will fall back to JSON
> automatically. Telemetry dashboards
> (`dashboards/grafana/torii_norito_rpc_observability.json`) and alert drills
> (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) remain mandatory before
> raising the stage, and the canary/smoke artefacts captured by
> `python/iroha_python/scripts/run_norito_rpc_smoke.sh` must be attached to the
> release ticket.

Before publishing:

1. Replace the **[stage: …]** marker with the stage advertised in Torii.
2. Link the release ticket to the latest stage report in
   `docs/source/torii/norito_rpc_stage_reports.md`.
3. Upload the Grafana/Alertmanager exports mentioned above along with the smoke
   bundle hashes from `scripts/run_norito_rpc_smoke.sh`.

This snippet keeps the OPS-NRPC release-note requirement satisfied without
forcing incident commanders to rephrase the rollout status every time.
