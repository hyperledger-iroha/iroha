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
     manifests in `specs/project_tracker/nexus_config_deltas/`.
3. **Configuration deltas**
   - All changes to `config/config.toml` require a ticket referencing the lane ID
     and dataspace alias.
   - Keep a redacted copy of the effective config in the ticket when the node
     joins or upgrades.
4. **Rollback drills**
   - Perform quarterly rollback rehearsals (stop node, restore previous bundle,
     replay config, re-run smoke). Record outcomes under
     `specs/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Compliance approvals**
   - Private/CBDC lanes must obtain compliance sign-off before changing DA
     policy or telemetry redaction knobs. Reference
     `specs/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. Telemetry & SLO Coverage

Dashboards and alert rules are versioned under `dashboards/` and documented in
`specs/nexus_telemetry_remediation_plan.md`. Operators MUST:

- Subscribe PagerDuty/on-call targets to `dashboards/alerts/nexus_audit_rules.yml`
  and the lane health rules under `dashboards/alerts/torii_norito_rpc_rules.yml`
  (covering Torii/Norito transport).
- Publish the following Grafana boards to the operations portal:
  - `nexus_lanes.json` (lane height, backlog, DA parity).
  - `nexus_settlement.json` (settlement latency, treasury deltas).
  - `android_operator_console.json` / SDK dashboards when the lane depends on
    mobile telemetry.
- Keep OTEL exporters aligned with `specs/torii/norito_rpc_telemetry.md`
  whenever Torii binary transport is enabled.
- Run the telemetry remediation checklist at least quarterly (Section 5 in
  `specs/nexus_telemetry_remediation_plan.md`) and attach the filled form
  to the ops review minutes.

### Key Metrics

| Metric | Description | Alert threshold |
|--------|-------------|-----------------|
| `nexus_lane_height{lane_id}` | Head height per lane; detects stalled validators. | Alert if no increase for 3 consecutive slots. |
| `nexus_da_backlog_chunks{lane_id}` | Unprocessed DA chunks per lane. | Alert above configured limit (default: 64 for public, 8 for private). |
| `nexus_settlement_latency_seconds{lane_id}` | Time between lane commit and global settlement. | Alert >900 ms P99 (public) or >1200 ms (private). |
| `torii_request_failures_total{scheme="norito_rpc"}` | Norito RPC error count. | Alert if 5-minute error ratio >2 %. |
| `telemetry_redaction_override_total` | Overrides issued for telemetry redaction. | Alert immediately (Sev 2) and require compliance ticket. |

### Multilane lifecycle and application evidence

- Treat `/v1/sumeragi/status` as the authoritative consensus status only.
  Inspect operational lane evidence through `/v1/sumeragi/diagnostics`.
  `native_amx_participant_applications` is ordered by route/incarnation and
  reports `certified_pending_carrier`, `committed_evidence_pending`,
  `durably_applied`, or `conflict`. A `conflict` row is a Sev 1 condition; do
  not select either same-height identity manually.
- Before approving drain or scale-in, require the diagnostics frontier to show
  no ordinary queue work, live reservation, certified-unmerged autonomous
  bundle, delayed work, pending merge entry, or unapplied/unverifiable Native
  control. Native participant controls are routing/settlement evidence only;
  economic effects become final once through the canonical global carrier.
- Do not remove lane directories, reservation journals, Native receipts,
  manifests, latest-index files, or autonomous sidecars by hand. Restart repair
  reconstructs bounded indexes and missing sidecars from authenticated finality
  evidence. Malformed, oversized, temporary, unexpected, or symlinked
  artefacts must remain fail-closed for incident capture.
- A recreated lane ID must have a new incarnation. After recreation, verify
  that diagnostics and archive paths contain no active reservation, QC,
  signing claim, marker, sidecar, or merge row from the retired incarnation
  before admitting traffic.
- Production transaction admission requires the durable queue-plan journal.
  If Torii reports `queue_plan_journal_outcome_unknown`, reconcile the exact
  transaction hash before retrying; a blind resubmission can obscure which
  authority owns the durable admission. A journal durability fault blocks
  drain until restart repair either restores the record or leaves the lane
  explicitly fail-closed.

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
  changelog entry in `specs/project_tracker/nexus_config_deltas/README.md`
  so auditors can track when requirements changed.

## 6. Related Resources

- `specs/nexus_overview.md` — architecture/high-level summary.
- `specs/nexus.md` — full technical specification.
- `specs/nexus_lanes.md` — lane geometry.
- `specs/nexus_transition_notes.md` — migration roadmap.
- `specs/cbdc_lane_playbook.md` — CBDC-specific policies.
- `specs/sora_nexus_operator_onboarding.md` — release/onboarding flow.
- `specs/nexus_telemetry_remediation_plan.md` — telemetry guardrails.

Keep these references up to date whenever roadmap item NX-14 advances or when
new lane classes, telemetry rules, or governance hooks are introduced.
