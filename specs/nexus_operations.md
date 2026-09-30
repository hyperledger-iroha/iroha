<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus Operations Runbook (NX-14)

**Roadmap link:** NX-14 — Nexus documentation & operator runbooks  
**Status:** Drafted 2026-03-24 — aligns with `specs/nexus_overview.md` and
the onboarding flow in `specs/sora_nexus_operator_onboarding.md`.  
**Audience:** Network operators, SRE/on-call engineers, governance coordinators.

This runbook summarises the operational lifecycle for Sora Nexus (Iroha 3)
nodes. It does not replace the deep specification (`specs/nexus.md`) or
lane-specific guides (e.g., `specs/cbdc_lane_playbook.md`), but gathers
the concrete checklists, telemetry hooks, and evidence requirements that must be
met before admitting or upgrading a node.

## 1. Operational Lifecycle

| Stage | Checklist | Evidence |
|-------|-----------|----------|
| **Pre-flight** | Validate artefact hashes/signatures, confirm `profile = "iroha3"`, and stage config templates. | Checksum log and signed manifest bundle. |
| **Catalog alignment** | Update `[nexus]` lane + dataspace catalog, routing policy, and DA thresholds to match the council-issued manifest. | `iroha3d --sora --config … --trace-config` output stored with ticket. |
| **Smoke & cutover** | Run `iroha3d --sora --config … --trace-config`, execute CLI smoke test (e.g., `FindNetworkStatus`), verify telemetry endpoints, then request admission. | Smoke-test log + Alertmanager silence confirmation. |
| **Steady state** | Monitor dashboards/alerts, rotate keys per governance cadence, and keep configs + runbooks in sync with manifest revisions. | Quarterly review minutes, linked dashboard screenshots, and rotation ticket IDs. |

Detailed onboarding instructions (including key replacement, routing policy
examples, and release-profile validation) live in
`specs/sora_nexus_operator_onboarding.md`. Reference that document whenever
artefact formats or scripts change.

## 2. Change Management & Governance Hooks

1. **Release updates**
   - Track announcements in `status.md` and `roadmap.md`.
   - Each release PR must attach the filled checklist from
     `specs/sora_nexus_operator_onboarding.md`.
2. **Lane manifest changes**
   - Governance publishes signed manifest bundles via the Space Directory.
   - Operators verify signatures, update catalog entries, and archive the
     manifests with the change ticket.
3. **Configuration deltas**
   - All changes to `config/config.toml` require a ticket referencing the lane ID
     and dataspace alias.
   - Keep a redacted copy of the effective config in the ticket when the node
     joins or upgrades.
4. **Rollback drills**
   - Perform quarterly rollback rehearsals (stop node, restore previous bundle,
     replay config, re-run smoke). Record outcomes in the change ticket.
5. **Compliance approvals**
   - Private/CBDC lanes must obtain compliance sign-off before changing DA
     policy or telemetry redaction knobs. Reference
     `specs/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. Telemetry & SLO Coverage

Dashboards and alert rules are versioned under `dashboards/`. Consensus status
comes from `/v1/sumeragi/status` and lane status from `/v1/sumeragi/lanes`
(`specs/telemetry.md`, `specs/sumeragi_lanes.md` §8). Operators MUST:

- Subscribe PagerDuty/on-call targets to `dashboards/alerts/nexus_audit_rules.yml`
  and the lane health rules under `dashboards/alerts/torii_norito_rpc_rules.yml`
  (covering Torii/Norito transport).
- Publish the following Grafana boards to the operations portal:
  - `nexus_lanes.json` (lane finality, oracle and buffer panels).
  - `settlement_router_overview.json` (settlement router buffers).
  - The Android SDK dashboards when the lane depends on mobile telemetry.
- Keep OTEL exporters aligned with `specs/torii/norito_rpc_telemetry.md`
  whenever Torii binary transport is enabled.

### Key Metrics

| Metric | Description | Alert threshold |
|--------|-------------|-----------------|
| `torii_request_failures_total{scheme="norito_rpc"}` | Norito RPC error count. | Alert if 5-minute error ratio >2 %. |

### Lane status

- `/v1/sumeragi/status` is the authoritative status of the global consensus
  instance. `/v1/sumeragi/lanes` lists the committed lane records and this
  node's lane instances (`specs/sumeragi_lanes.md` §8). A lane that holds work
  but stops advancing its merged frontier is a Sev 2 condition.
- Lanes are opened, closed and scaled by the governed `sumeragi_lane_policy`
  and deterministic autoscale in the global chain (`specs/sumeragi_lanes.md`
  §2, §6). A recreated lane is a new incarnation; never remove lane storage by
  hand.

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
- **Runbook versioning:** Record every significant change to this file in the
  change ticket so auditors can track when requirements changed.

## 6. Related Resources

- `specs/nexus_overview.md` — architecture/high-level summary.
- `specs/nexus.md` — full technical specification.
- `specs/nexus_lanes.md` — lane catalog geometry and storage.
- `specs/sumeragi_lanes.md` — lane instances, merge and autoscale.
- `specs/cbdc_lane_playbook.md` — CBDC-specific policies.
- `specs/sora_nexus_operator_onboarding.md` — release/onboarding flow.

Keep these references up to date whenever roadmap item NX-14 advances or when
new lane classes, telemetry rules, or governance hooks are introduced.
