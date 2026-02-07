---
lang: my
direction: ltr
source: docs/source/soranet/snnet10_stage_gate_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 987d79706868ea9229e7525fb88773c02ddb93dcaccd83ac87553d86286023fd
source_last_modified: "2025-12-29T18:16:36.198347+00:00"
translation_last_reviewed: 2026-02-07
title: SNNet-10 Stage-Gate Reporting Template
summary: Reporting structure, attachment checklist, and sample form for promoting SNNet-10 between phases.
---

# SNNet-10 Stage-Gate Reporting Template

This template keeps **SNNet-10** promotions deterministic. Every time the
network advances from one phase to the next (T0 → T1 or T1 → T2), the sponsoring
team submits a signed report to the Governance Council. The report references
telemetry artefacts, onboarding kits, and drill evidence so approvals and
rollbacks can be audited later.

Use this guide together with:

- [`docs/source/soranet/testnet_rollout_plan.md`](testnet_rollout_plan.md) —
  authoritative scope, metrics, and verification checklist.
- [`docs/source/soranet/snnet10_beta_onboarding.md`](snnet10_beta_onboarding.md)
  — operator inputs for the public beta (T1).
- `cargo xtask soranet-testnet-kit` and
  `cargo xtask soranet-testnet-metrics` — generate the deterministic kit and
  evaluate success metrics.
- `cargo xtask soranet-testnet-feed` — assemble the JSON feed (metadata, relays,
  metrics, drill evidence, attachment hashes) that governance ingest tools use
  when reviewing stage-gate submissions.

## Stage overview

| Promotion | Target scope | Primary evidence |
|-----------|--------------|------------------|
| **T0 → T1 (Closed → Public Beta)** | ≥100 relays, PQ guard rotation enabled, exit bonding live, SDK betas default to `anon-guard-pq`. | Completed onboarding kits, 14-day metrics window, guard rotation transcripts, exit bonding receipts, signed brownout/downgrade drill bundle. |
| **T1 → T2 (Public Beta → Mainnet Default)** | Network defaults to SoraNet, PQ ratchet enforcement, MASQUE/obfs transports ready, rollback tested. | All T1 inputs plus staging-to-production delta analysis, MASQUE readiness report, signed downgrade communication plan, and operator sign-off for automatic SoraNet default. |

> **Determinism:** Every attachment referenced below must include a SHA-256
> fingerprint. Use `sha256sum <file>` or the checksum manifest emitted by build
> tooling.

## Required inputs

1. **Metrics snapshot**
   - 14 consecutive days in the schema used by
     `docs/examples/soranet_testnet_operator_kit/07-metrics-sample.json`.
   - Validation report from
     `cargo xtask soranet-testnet-metrics --input <snapshot> --out reports/metrics-report.json`.
2. **Guard rotation evidence**
   - Transcript or CI artefact from
     `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`
     (one per relay cohort promoted in this gate).
3. **PQ supply summary**
   - Aggregated `sorafs_orchestrator_pq_ratio`,
     `sorafs_orchestrator_pq_candidate_ratio`,
     `sorafs_orchestrator_pq_deficit_ratio`, and
     `sorafs_orchestrator_brownouts_total` metrics covering the same window.
4. **Brownout/downgrade drills**
   - At least one drill per region, with Alertmanager IDs, timestamps, and
   - Sign the drill log plus attachments with `cargo xtask soranet-testnet-drill-bundle` so the feed records `drill_log.signed = true`.
     mitigation steps (<5 minutes to recover).
5. **Exit bonding receipts**
   - Signed bond manifest and treasury acknowledgement for every relay being
     whitelisted.
6. **Compliance attestations**
   - Latest opt-out catalogue hash (`governance/compliance/soranet_opt_outs.json`)
     and jurisdiction attestations gathered via the onboarding kit.
7. **Change management artefacts (T1 → T2 only)**
   - MASQUE/obfs transport readiness report.
   - Signed rollback plan (DNS + guard cache invalidation + SDK comms) referring
     to `docs/source/soranet/templates/downgrade_communication_template.md`.

## Report structure

Mirror the following structure in your submission. A ready-to-copy Markdown file
lives in `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

### 1. Metadata

| Field | Example |
|-------|---------|
| Promotion | `T0 → T1` |
| Date window | `2026-11-01 → 2026-11-14` |
| Relays covered | `42 existing + 65 new (list IDs in appendix)` |
| Primary contact | `relay-ops@sora.foundation` |
| Submission hash | `sha256:...` (zip/tar archive hash) |

### 2. Metrics summary

| Metric | Observed | Threshold | Pass? | Source |
|--------|----------|-----------|-------|--------|
| Circuit success ratio | `0.972` | `≥ 0.95` | ✅ | `reports/metrics-report.json` |
| Fetch brownout ratio | `0.006` | `≤ 0.01` | ✅ | `reports/metrics-report.json` |
| GAR mix variance | `+4.2%` max | `≤ ±10%` | ✅ | `reports/metrics-report.json` |
| PoW p95 seconds | `2.4s` | `≤ 3s` | ✅ | `telemetry/pow_window.json` |
| Latency p95 | `178 ms` | `<200 ms` | ✅ | `telemetry/latency_window.json` |

Add narrative explaining anomalies, mitigation, or policy overrides. Reference
Alertmanager IDs and runbooks when brownouts or downgrades occurred.

### 3. Drill & incident log

List each drill/incident with timestamps, severity, and links to logs, e.g.:

```
| Timestamp (UTC) | Region | Event | Alert ID | Mitigation |
| 2026-11-05 03:17 | NRT | Brownout drill | alert://soranet/brownout/1234 | Switched to anon-guard-pq within 3m12s |
```

### 4. Attachments & hashes

Provide a table enumerating every artefact referenced in the report:

| Artefact | Path | SHA-256 |
|----------|------|---------|
| Metrics snapshot | `reports/metrics-window.json` | `...` |
| Metrics report | `reports/metrics-report.json` | `...` |
| Guard rotation transcripts | `evidence/guard_rotation/*.log` | `...` |
| Exit bonding manifests | `evidence/exit_bonds/*.to` | `...` |
| Drill logs | `evidence/drills/*.md` | `...` |
| MASQUE readiness (T1→T2) | `reports/masque-readiness.md` | `...` |

### 5. Approvals

Capture sign-off from the Networking TL, Governance representative, and SRE
delegate. Include signature artefacts when required by policy.

## Submission workflow

1. Package the filled template, attachments, and checksum manifest (e.g.,
   `snnet10-stage-gate-2026Q4.tar.zst` + `checksums.sha256`).
2. Upload per governance instructions (helpdesk ticket + secure upload).
3. Record the submission in the governance tracker with hashes and review due
   date (two business days by default).
4. Governance replies with approval/rejection and any remediation actions.
5. Once approved, archive the signed package in your CMDB and reference it in
   the next weekly digest (`docs/source/status/soranet_testnet_weekly_digest.md`).

## Example files

Copy `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`
when preparing your submission. The example is wired to the same section
headings used above so automation (lint/checklist scripts) can parse the report.
Sample JSON feed emitted by `cargo xtask soranet-testnet-feed` lives at
`docs/examples/soranet_testnet_stage_gate/feed-sample.json`.

## Automation

Use the helper command to emit the verification feed once every artefact is in
place. Sign drills first:

```bash
cargo xtask soranet-testnet-drill-bundle \
  --log evidence/drills-log.json \
  --signing-key drill_signing_key_ed25519.hex \
  --promotion T0->T1 \
  --window-start 2026-11-01 \
  --window-end 2026-11-14 \
  --attachment guard-rotation=evidence/guard_rotation.log \
  --attachment exit-bond=evidence/exit_bond_manifest.to \
  --out evidence/drills-signed.json
```

Then emit the feed:

```bash
cargo xtask soranet-testnet-feed \
  --promotion T0->T1 \
  --window-start 2026-11-01 \
  --window-end 2026-11-14 \
  --relay relay-sjc-01 --relay relay-nrt-02 \
  --metrics-report reports/metrics-report.json \
  --drill-log evidence/drills-signed.json \
  --stage-report reports/stage-gate.md \
  --attachment guard-rotation=evidence/guard_rotation.log \
  --attachment exit-bond=evidence/exit_bond_manifest.to \
  --out reports/snnet10_verification_feed.json
```

The command embeds the metrics report, optional drill log, stage report, and
attachment hashes into a deterministic JSON object so Governance can verify
submissions quickly. Signed drill bundles set `drill_log.signed: true` in the
feed. Attachments can be repeated (`--attachment label=path`)
for every artefact referenced in the stage-gate template.
