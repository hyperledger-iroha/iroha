---
lang: hy
direction: ltr
source: docs/source/soranet/snnet10_beta_onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba456370b8d97e4596bbd1357e10967a78813a820dea0c2091a3dbf5b4cab8c2
source_last_modified: "2025-12-29T18:16:36.197869+00:00"
translation_last_reviewed: 2026-02-07
title: SNNet-10 Public Beta Onboarding Brief
summary: Checklist and submission workflow for operators joining the SNNet-10 public beta (phase T1).
---

# SNNet-10 Public Beta Onboarding Brief

The SNNet-10 public beta (phase **T1**) expands the SoraNet anonymity overlay
from the closed operator pool (T0) to a wider relay set (≥100 relays across at
least three autonomous systems). This brief distils the rollout plan in
`docs/source/soranet/testnet_rollout_plan.md` into an actionable onboarding
checklist for new beta operators.

Key expectations for T1:

- Guard rotation is mandatory; relays must prove adherence to the published
  guard hash chain.
- Exit bonding is enforced. Admission packages must include the bond manifest
  and signed treasury proof.
- SDK beta builds default to `anon-guard-pq`. Your relay must sustain the PQ
  supply targets that keep brownouts <1%.
- Governance requires deterministic artefacts (kit, metrics report, signed drill bundle)
  before whitelisting descriptors.

## Prerequisites

Ensure the following before requesting beta access (adapted from the T0 kit in
`docs/examples/soranet_testnet_operator_kit/`):

1. **Hardware & network**
   - 8+ physical cores, 16 GiB RAM, NVMe storage sustaining ≥500 MiB/s.
   - Dual-stack connectivity (IPv4 + IPv6) with QUIC/UDP 443 permitted end to
     end. Upstream shapers must preserve ≥20 Mbps during brownout drills.
2. **Identity & compliance**
   - Relay identity keys backed by an HSM or dedicated enclave.
   - Latest opt-out catalogue merged into the orchestrator configuration (see
     the SNNet-9 compliance block embedded in
     `docs/examples/soranet_testnet_operator_kit/03-config-example.toml`).
   - Jurisdictional attestations captured for every region you serve.
3. **Telemetry**
   - Prometheus/OTLP exporters for `sorafs.fetch.*`, `soranet_privacy_*`, and
     `sorafs_orchestrator_*` metrics as listed in
     `docs/source/soranet/testnet_rollout_plan.md`.
   - Grafana/Alertmanager templates imported from
     `dashboard_templates/soranet_testnet_overview.json` and
     `alert_templates/soranet_testnet_rules.yml`.
4. **Operational readiness**
   - Brownout/downgrade drill schedule on file with the Governance helpdesk.
   - Incident escalation contacts validated through a live paging test.

## Generate the onboarding kit

Use the workspace helper to materialise the deterministic kit:

```bash
cargo xtask soranet-testnet-kit --out build/snnet10-kit
```

The command copies the canonical files from
`xtask/templates/soranet_testnet/` (rendered example:
`docs/examples/soranet_testnet_operator_kit/`). Edit only the operator input
sections:

- `02-checklist.md` — tick every prerequisite (hardware, compliance, guard
  rotation smoke, telemetry dry run).
- `03-config-example.toml` — overlay your relay IDs, guard seeds, exit bonding,
  and telemetry endpoints while preserving the supplied defaults.
- `04-telemetry.md` — replace sample screenshots with captures from your live
  dashboards/alerts.
- `05-incident-playbook.md` — record your local escalation contacts and
  mitigation timings.
- `06-verification-report.md` — summarise drill outputs, PQ ratios, and any
  deviations that required governance approval.

Archive the completed kit (for example `snnet10-beta-<operator>.tar.zst`) when
submitting to governance so reviewers can diff against the canonical sources.

## Telemetry & verification deliverables

The beta gate requires a 14-day window of measurements:

1. **Metrics snapshot**
   - Export counters matching the schema used by
     `docs/examples/soranet_testnet_operator_kit/07-metrics-sample.json`.
   - Run the validator:
     ```bash
     cargo xtask soranet-testnet-metrics \
       --input path/to/metrics.json \
       --out reports/metrics-report.json
     ```
   - Attach the generated report plus the raw snapshot. Failures must include a
     remediation plan before reapplying.
2. **Guard rotation proof**
   - Provide the rotation transcript emitted by
     `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` or
     the equivalent CI artefact keyed by your relay ID.
3. **Exit bonding evidence**
   - Include the signed bond manifest and treasury acknowledgement referenced in
     your descriptor submission.
4. **Brownout/downgrade drill log**
   - Capture at least one drill per region with corresponding alert IDs and
     mitigation timestamps (<5 minutes to recovery).
5. **PQ supply audit**
   - Export `sorafs_orchestrator_pq_ratio`,
     `sorafs_orchestrator_pq_candidate_ratio`, and
     `sorafs_orchestrator_brownouts_total` summaries covering the same 14-day
     window. Highlight any intentional deficits (e.g., scheduled guard swaps).

## Submission workflow

1. Email or upload (per governance instructions) the archive containing:
   - Completed onboarding kit files.
   - `reports/metrics-report.json` + raw metrics snapshot.
   - Guard rotation, exit bonding, and drill evidence.
   - Contact roster + escalation confirmations.
   - The JSON feed emitted by
     `cargo xtask soranet-testnet-feed` (captures promotion metadata, relay
     roster, metrics report hash, drill summaries, and attachment SHA-256
     fingerprints for audit automation).
2. Log the request in the governance tracker with references to the artefacts’
   hashes (use `sha256sum <file>`). Attach the report IDs that map to your
   dashboards so reviewers can replay the data.
3. Governance reviews the package, validates metrics, and whitelists your relay
   descriptors. Expect follow-up questions within two business days.
4. After approval, mirror the final artefacts in your internal CMDB. Governance
   archives a signed copy alongside the SNNet-10 status digest.

## Escalation & support

- **Helpdesk:** Continue to route questions and promotion requests through the
  governance helpdesk noted in `docs/source/soranet/testnet_rollout_plan.md`.
- **Status cadence:** Once admitted, submit weekly deltas using
  `docs/source/status/soranet_testnet_weekly_digest.md` to keep the SNNet-10
  steering review up to date.
- **Changes:** Any modification to exit bonding, jurisdiction, or telemetry
  coverage requires an updated kit submission referencing the previous approval
  ID.

Keep this brief synchronized with the roadmap: updates to telemetry, compliance
policy, or the onboarding kit must be reflected here in the same pull request.
