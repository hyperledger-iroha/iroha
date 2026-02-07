---
lang: zh-hant
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0864191061aa50c474a6c42eaa98a637d0f58d8d2e1ba12876c729e57253877
source_last_modified: "2025-12-29T18:16:36.029352+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Torii Norito-RPC Canary Runbook (NRPC-2C)

This runbook operationalises the **NRPC-2** rollout plan by describing how to
promote the Norito-RPC transport from staging lab validation into the production
“canary” stage. It should be read alongside:

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md) (protocol contract)
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## Roles & Inputs

| Role | Responsibility |
|------|----------------|
| Torii Platform TL | Approves config deltas, signs off on smoke tests. |
| NetOps | Applies ingress/envoy changes and monitors canary pool health. |
| Observability liaison | Verifies dashboards/alerts and captures evidence. |
| Platform Ops | Drives change ticket, coordinates rollback rehearsal, updates trackers. |

Required artefacts:

- Latest `iroha_config` Norito patch with `transport.norito_rpc.stage = "canary"` and
  `transport.norito_rpc.allowed_clients` populated.
- Envoy/Nginx config snippet that preserves `Content-Type: application/x-norito` and
  enforces the canary client mTLS profile (`defaults/torii_ingress_mtls.yaml`).
- Token allowlist (YAML or Norito manifest) for the canary clients.
- Grafana URL + API token for `dashboards/grafana/torii_norito_rpc_observability.json`.
- Access to the parity smoke harness
  (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) and alert drill script
  (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`).

## Pre-flight Checklist

1. **Spec freeze confirmed.** Ensure `docs/source/torii/nrpc_spec.md` hash matches the
   last signed release and no pending PRs touch the Norito header/layout.
2. **Config validation.** Run
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
   to confirm the new `transport.norito_rpc.*` entries parse.
3. **Scheme caps.** Set conservative `torii.preauth_scheme_limits.norito_rpc`
   (e.g., 25 concurrent connections) so binary callers cannot starve JSON traffic.
4. **Ingress rehearsal.** Apply the Envoy patch in staging, replay the negative test
   (`cargo test -p iroha_torii -- norito_ingress`) and confirm stripped headers are
   rejected with HTTP 415.
5. **Telemetry sanity.** In staging, run `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` and attach the generated evidence bundle.
6. **Token inventory.** Verify the canary allowlist includes at least two operators
   per region; store the manifest in `artifacts/norito_rpc/<YYYYMMDD>/allowlist.json`.
7. **Ticketing.** Open the change ticket with start/end window, rollback plan, and
   links to this runbook plus the telemetry evidence.

## Canary Promotion Procedure

1. **Apply config patch.**
   - Roll out the `iroha_config` delta (stage=`canary`, allowlist populated,
     scheme limits set) through admission.
   - Restart or hot-reload Torii, confirming the patch is acknowledged via
     `torii.config.reload` logs.
2. **Update ingress.**
   - Deploy the Envoy/Nginx config enabling the Norito header routing/mTLS profile
     for the canary pool.
   - Verify `curl -vk --cert <client.pem>` responses include the Norito `X-Iroha-Error-Code`
     headers when expected.
3. **Smoke tests.**
   - Execute `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary`
     from the canary bastion. Capture JSON + Norito transcripts and store them under
     `artifacts/norito_rpc/<YYYYMMDD>/smoke/`.
   - Record hashes inside `docs/source/torii/norito_rpc_stage_reports.md`.
4. **Observe telemetry.**
   - Watch `torii_active_connections_total{scheme="norito_rpc"}` and
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` for at least 30 minutes.
   - Export the Grafana dashboard via API and attach it to the change ticket.
5. **Alert rehearsal.**
   - Run `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` to inject
     malformed Norito envelopes; ensure Alertmanager records the synthetic incident
     and clears automatically.
6. **Evidence capture.**
   - Update `docs/source/torii/norito_rpc_stage_reports.md` with:
     - Config digest
     - Allowlist manifest hash
     - Smoke test timestamp
     - Grafana export checksum
     - Alert drill ID
   - Upload artefacts to `artifacts/norito_rpc/<YYYYMMDD>/`.

## Monitoring & Exit Criteria

Remain in canary until all of the following are true for ≥72 hours:

- Error rate (`torii_request_failures_total{scheme="norito_rpc"}`) ≤1 % and no sustained
  spikes in `torii_norito_decode_failures_total`.
- Latency parity (`p95` Norito vs JSON) within 10 %.
- Alert dashboard quiet except for scheduled drills.
- Operators in the allowlist submit parity reports with no schema mismatches.

Document daily status in the change ticket and capture snapshots in
`docs/source/status/norito_rpc_canary_log.md` (if present).

## Rollback Procedure

1. Flip `transport.norito_rpc.stage` back to `"disabled"` and clear
   `allowed_clients`; apply via admission.
2. Remove the Envoy/Nginx route/mTLS stanza, reload proxies, and confirm new Norito
   connections are refused.
3. Revoke canary tokens (or deactivate bearer credentials) so outstanding sessions drop.
4. Watch `torii_active_connections_total{scheme="norito_rpc"}` until it reaches zero.
5. Rerun the JSON-only smoke harness to ensure baseline functionality.
6. File a post-mortem stub under `docs/source/postmortems/norito_rpc_rollback.md`
   within 24 hours and update the change ticket with impact summary + metrics.

## Post-Canary Close-Out

When exit criteria are satisfied:

1. Update `docs/source/torii/norito_rpc_stage_reports.md` with the GA recommendation.
2. Add a `status.md` entry summarising canary outcomes and evidence bundles.
3. Notify SDK leads so they can switch staging fixtures to Norito for parity runs.
4. Prepare the GA config patch (stage=`ga`, remove allowlist) and schedule the
   promotion per the NRPC-2 plan.

Following this runbook ensures every canary promotion collects the same evidence,
keeps rollback deterministic, and satisfies the NRPC-2 roadmap acceptance
criteria.
