---
lang: am
direction: ltr
source: docs/source/nexus_operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 413953b2ca41875bb960be56995aa03dbaa58af4a30f927c24d1e8815c7da472
source_last_modified: "2025-12-29T18:16:35.991294+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus Operations Runbook (NX-14)

**Roadmap link:** NX-14 — Nexus documentation & operator runbooks  
**Status:** Drafted 2026-03-24 — aligns with `docs/source/nexus_overview.md` and
the onboarding flow in `docs/source/sora_nexus_operator_onboarding.md`.  
**Audience:** Network operators, SRE/on-call engineers, governance coordinators.

This runbook summarises the operational lifecycle for Sora Nexus (Iroha 3)
nodes. It does not replace the deep specification (`docs/source/nexus.md`) or
lane-specific guides (e.g., `docs/source/cbdc_lane_playbook.md`), but gathers
the concrete checklists, telemetry hooks, and evidence requirements that must be
met before admitting or upgrading a node.

## 1. Operational Lifecycle

| Stage | Checklist | Evidence |
|-------|-----------|----------|
| **Pre-flight** | Validate artefact hashes/signatures, confirm `profile = "iroha3"`, and stage config templates. | `scripts/select_release_profile.py` output, checksum log, signed manifest bundle. |
| **Catalog alignment** | Update `[nexus]` lane + dataspace catalog, routing policy, and DA thresholds to match the council-issued manifest. | `irohad --sora --config … --trace-config` output stored with ticket. |
| **Smoke & cutover** | Run `irohad --sora --config … --trace-config`, execute CLI smoke test (e.g., `FindNetworkStatus`), verify telemetry endpoints, then request admission. | Smoke-test log + Alertmanager silence confirmation. |
| **Steady state** | Monitor dashboards/alerts, rotate keys per governance cadence, and keep configs + runbooks in sync with manifest revisions. | Quarterly review minutes, linked dashboard screenshots, and rotation ticket IDs. |

Detailed onboarding instructions (including key replacement, routing policy
examples, and release-profile validation) live in
`docs/source/sora_nexus_operator_onboarding.md`. Reference that document whenever
artefact formats or scripts change.

## 2. Change Management & Governance Hooks

1. **Release updates**
   - Track announcements in `status.md` and `roadmap.md`.
   - Each release PR must attach the filled checklist from
     `docs/source/sora_nexus_operator_onboarding.md`.
2. **Lane manifest changes**
   - Governance publishes signed manifest bundles via the Space Directory.
   - Operators verify signatures, update catalog entries, and archive the
     manifests in `docs/source/project_tracker/nexus_config_deltas/`.
3. **Configuration deltas**
   - All changes to `config/config.toml` require a ticket referencing the lane ID
     and dataspace alias.
   - Keep a redacted copy of the effective config in the ticket when the node
     joins or upgrades.
4. **Rollback drills**
   - Perform quarterly rollback rehearsals (stop node, restore previous bundle,
     replay config, re-run smoke). Record outcomes under
     `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Compliance approvals**
   - Private/CBDC lanes must obtain compliance sign-off before changing DA
     policy or telemetry redaction knobs. Reference
     `docs/source/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. Telemetry & SLO Coverage

Dashboards and alert rules are versioned under `dashboards/` and documented in
`docs/source/nexus_telemetry_remediation_plan.md`. Operators MUST:

- Subscribe PagerDuty/on-call targets to `dashboards/alerts/nexus_audit_rules.yml`
  and the lane health rules under `dashboards/alerts/torii_norito_rpc_rules.yml`
  (covering Torii/Norito transport).
- Publish the following Grafana boards to the operations portal:
  - `nexus_lanes.json` (lane height, backlog, DA parity).
  - `nexus_settlement.json` (settlement latency, treasury deltas).
  - `android_operator_console.json` / SDK dashboards when the lane depends on
    mobile telemetry.
- Keep OTEL exporters aligned with `docs/source/torii/norito_rpc_telemetry.md`
  whenever Torii binary transport is enabled.
- Run the telemetry remediation checklist at least quarterly (Section 5 in
  `docs/source/nexus_telemetry_remediation_plan.md`) and attach the filled form
  to the ops review minutes.

### Key Metrics

| Metric | Description | Alert threshold |
|--------|-------------|-----------------|
| `nexus_lane_height{lane_id}` | Head height per lane; detects stalled validators. | Alert if no increase for 3 consecutive slots. |
| `nexus_da_backlog_chunks{lane_id}` | Unprocessed DA chunks per lane. | Alert above configured limit (default: 64 for public, 8 for private). |
| `nexus_settlement_latency_seconds{lane_id}` | Time between lane commit and global settlement. | Alert >900 ms P99 (public) or >1200 ms (private). |
| `torii_request_failures_total{scheme="norito_rpc"}` | Norito RPC error count. | Alert if 5-minute error ratio >2 %. |
| `telemetry_redaction_override_total` | Overrides issued for telemetry redaction. | Alert immediately (Sev 2) and require compliance ticket. |

## 4. Incident Response

| Severity | Definition | Required actions |
|----------|------------|------------------|
| **Sev 1** | Data-space isolation breach, settlement halt >15 min, or governance vote corruption. | Page Nexus Primary + Release Engineering + Compliance. Freeze lane admission, gather metrics/logs, publish incident comms within 60 min, file RCA in ≤5 business days. |
| **Sev 2** | Lane backlog exceeding SLA, telemetry blind spot >30 min, failed manifest rollout. | Page Nexus Primary + SRE, mitigate within 4 h, capture follow-up issues within 2 business days. |
| **Sev 3** | Non-blocking regressions (docs drift, alert misfire). | Log in tracker, schedule fix within sprint. |

Incident tickets must include:

1. Affected lane/data-space IDs and manifest hashes.
2. Timeline (UTC) with detection, mitigation, recovery, and communications.
3. Metrics/screenshots supporting detection.
4. Follow-up tasks (with owners/dates) and whether automation/runbooks need
   updates.

## 5. Evidence & Audit Trail

- **Artefact archive:** Store bundles, manifests, and telemetry exports under
  `artifacts/nexus/<lane>/<date>/`.
- **Config snapshots:** Redacted `config.toml` plus `trace-config` output for
  each release.
- **Governance linkage:** Council meeting notes and signed decisions referenced
  in the onboarding or incident ticket.
- **Telemetry exports:** Weekly snapshots of Prometheus TSDB chunks related to
  the lane, attached to the audit share for 12 months minimum.
- **Runbook versioning:** Every significant change to this file must include a
  changelog entry in `docs/source/project_tracker/nexus_config_deltas/README.md`
  so auditors can track when requirements changed.

## 6. Related Resources

- `docs/source/nexus_overview.md` — architecture/high-level summary.
- `docs/source/nexus.md` — full technical specification.
- `docs/source/nexus_lanes.md` — lane geometry.
- `docs/source/nexus_transition_notes.md` — migration roadmap.
- `docs/source/cbdc_lane_playbook.md` — CBDC-specific policies.
- `docs/source/sora_nexus_operator_onboarding.md` — release/onboarding flow.
- `docs/source/nexus_telemetry_remediation_plan.md` — telemetry guardrails.

Keep these references up to date whenever roadmap item NX-14 advances or when
new lane classes, telemetry rules, or governance hooks are introduced.
