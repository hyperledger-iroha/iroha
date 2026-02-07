---
lang: hy
direction: ltr
source: docs/source/sorafs_gateway_dns_design_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1f8de6c5ff283479cd76dc60afbdc4bc142596f3e48c52472d2a943ec805b0d
source_last_modified: "2025-12-29T18:16:36.144614+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway & DNS Kickoff Runbook
summary: Roles, automation drills, and evidence requirements for the 2025-03 gateway/DNS design session (roadmap Decentralized DNS & Gateway workstream).
---

# 1. Purpose & Scope

This runbook turns the “owner runbooks + automation rehearsal” requirement from
`roadmap.md` (Decentralized DNS & Gateway entry under **Near-Term Execution**) into
actionable steps. It binds the SF‑4 (deterministic DNS) and SF‑5 (gateway
hardening) deliverables by describing how the Networking, Ops, QA, and Docs
teams rehearse the automation stack ahead of the 2025‑03‑03 kickoff.

- **What it covers:** deterministic host derivation, SoraDNS directory releases,
  TLS/GAR automation probes, conformance harness execution, telemetry capture,
  and evidence packaging.
- **What it does not cover:** production incident response (see
  `docs/source/sorafs_gateway_operator_playbook.md`) or post-kickoff roadmap
  changes (tracked directly in `roadmap.md` and `status.md`).

# 2. Owner & Role Matrix

| Workstream | Responsibilities | Primary / Backup | Required Artefacts |
|------------|------------------|------------------|--------------------|
| Networking TL (DNS stack) | Maintain deterministic host plan, run RAD directory releases, own resolver telemetry inputs. | networking@sora | `artifacts/soradns_directory/<timestamp>/`, `docs/source/soradns/deterministic_hosts.md` diff, RAD release metadata. |
| Ops Automation Lead (gateway) | Execute TLS/ECH/GAR automation drills, operate `sorafs-gateway-probe`, manage PagerDuty hooks. | ops@sorafs / sre@sora | `artifacts/sorafs_gateway_probe/<timestamp>/`, probe JSON, updated `ops/drill-log.md`. |
| QA Guild & Tooling WG | Run `ci/check_sorafs_gateway_conformance.sh`, curate fixtures, and archive Norito self-cert bundles. | qa.guild@sorafs / tooling@sora | `artifacts/sorafs_gateway_conformance/<timestamp>/`, `artifacts/sorafs_gateway_attest/<timestamp>/`. |
| Docs / DevRel | Capture minutes, update `docs/source/sorafs_gateway_dns_design_*` files, publish evidence summary in portal. | docs@sora / devrel@sora | Updated pre-read, runbook changelog, telemetry appendix, agenda action register. |

# 3. Inputs & Prerequisites

- Deterministic host spec (`docs/source/soradns/deterministic_hosts.md`) and the
  resolver attestation scaffolding (`docs/source/soradns/resolver_attestation_directory.md`).
- Gateway artefacts: profile, deployment handbook, TLS automation, direct-mode
  guidance, and self-cert workflow (see `docs/source/sorafs_gateway_*` docs).
- Tooling: `cargo xtask` helpers (`soradns-directory-release`,
  `sorafs-gateway-probe`), `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, and CI helpers
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets & keys: GAR release key, DNS/TLS ACME credentials, PagerDuty routing
  key, Torii auth token for resolver fetches.

# 4. Pre-flight Checklist

1. **Confirm attendees & agenda:** Update
   `docs/source/sorafs_gateway_dns_design_attendance.md` and share the current
   `docs/source/sorafs_gateway_dns_design_agenda.md`.
2. **Stage artefact directories:** Create
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` and
   `artifacts/soradns_directory/<YYYYMMDD>/` to store rehearsal outputs.
3. **Refresh fixtures:** Pull the latest GAR manifests, RAD proofs, and gateway
  conformance fixtures (`git submodule update --init --recursive` if required).
4. **Secrets sanity check:** Verify the release Ed25519 key, ACME account file,
  and PagerDuty token exist and have matching checksums in the vault record.
5. **Telemetry targets:** Ensure the staging Pushgateway endpoint and the GAR
  Grafana board (`dashboards/grafana/sorafs_fetch_observability.json`) are
  reachable before the drill begins.

# 5. Automation Rehearsal Steps

Each step MUST be executed and archived before the kickoff. Store command
outputs under the run ID directory created in step 2.

## 5.1 Deterministic Host Map & RAD Directory Release

1. Run the deterministic host derivation script (Rust or JS recipe) against the
   proposed manifest set to ensure no drift from
   `docs/source/soradns/deterministic_hosts.md`.
2. When GAR envelopes already exist, verify the host patterns match the derived
   output before rehearsals:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --verify-host-patterns artifacts/soradns_gar/docs.sora.json
```

3. Generate a resolver directory bundle:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

4. Record the printed directory ID, SHA-256, and output paths in
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` plus the kickoff
   meeting notes.

## 5.2 DNS Telemetry Capture

1. Tail resolver transparency logs for at least ten minutes:

```bash
./scripts/telemetry/run_soradns_transparency_tail.sh \
  --log /var/log/soradns/transparency.jsonl \
  --metrics artifacts/sorafs_gateway_dns_design_metrics_20250302.prom \
  --output artifacts/sorafs_gateway_dns_design_transparency_20250302.jsonl \
  --push-url https://pushgateway.stg.sora.net/metrics/job/soradns
```

2. Append a summary (hashes, proof counts, freshness window) to
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.

## 5.3 Gateway TLS / GAR Probe

1. Execute the TLS automation drill per
   `docs/source/sorafs_gateway_tls_automation.md`.
2. Run the probe with drill flags so PagerDuty/webhook payloads are exercised:

```bash
cargo xtask sorafs-gateway-probe \
  --target https://gateway.stg.sora \
  --report-json artifacts/sorafs_gateway_probe/20250302/report.json \
  --summary-json artifacts/sorafs_gateway_probe/20250302/summary.json \
  --drill-name dns-gateway-kickoff \
  --pagerduty-routing-key "${PD_KEY}" \
  --pagerduty-detail playbook=docs/source/sorafs_gateway_tls_automation.md \
  --ops-notes artifacts/sorafs_gateway_dns/20250302/ops-notes.md
```

3. If the drill uses CI, invoke `ci/check_sorafs_gateway_probe.sh` to keep the
   dashboards/tests current. Copy the resulting `ops/drill-log` entry into the
   kickoff minutes.

## 5.4 Conformance Harness & Self-Cert

1. Run the conformance suite:

```bash
CI=1 ci/check_sorafs_gateway_conformance.sh \
  --out artifacts/sorafs_gateway_conformance/20250302
```

2. Produce a Norito attestation bundle for the rehearsal gateway:

```bash
./scripts/sorafs_gateway_self_cert.sh \
  --config docs/examples/sorafs_gateway_self_cert.conf \
  --out artifacts/sorafs_gateway_attest/20250302
```

3. Attach `sorafs_gateway_report.json` and `.to` envelopes to the evidence
   bundle; cross-link them from `docs/source/sorafs_gateway_dns_design_attendance.md`
   once review notes are added.

## 5.5 Evidence Bundle Assembly

Create `artifacts/sorafs_gateway_dns/20250302/runbook_bundle/` with:

- RAD directory JSON, record, metadata, and resolver `.norito` copies.
- Transparency `.jsonl` and `.prom` files with SHA-256 manifests.
- Probe JSON + screenshots, CI logs, PagerDuty payloads.
- Conformance output (`report.json`, logs) and self-cert envelope.
- Signed meeting agenda, attendee list snapshot, and action register.

Zip the directory (`tar -czf sorafs_gateway_dns_runbook_20250302.tgz ...`) and
upload it alongside the pre-read updates.

### Hash tracker (fill during upload)

| Artifact | SHA-256 | Upload path |
|----------|---------|-------------|
| `sorafs_gateway_dns_runbook_20250302.tgz` | `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0` | `s3://sora-governance/sorafs/gateway_dns/20250302/` |
| `gateway_dns_minutes_20250302.pdf` | `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18` | `s3://sora-governance/sorafs/gateway_dns/20250302/` |
| Additional screenshots / evidence | n/a (not captured for this rehearsal) | — |

## 5.6 Alert bundle verification

- Load the DNS/gateway alert pack (`dashboards/alerts/soradns_gateway_rules.yml`) into the staging
  Alertmanager and run `promtool check rules dashboards/alerts/soradns_gateway_rules.yml` as part of
  the rehearsal. The pack watches resolver proof age/TTL, CID drift, sync lag, alias cache refresh
  failures, and gateway TLS renewal/expiry so operators have guardrails during promotions.
- Execute the fixture-backed alert tests with
  `promtool test rules dashboards/alerts/tests/soradns_gateway_rules.test.yml` (or
  `scripts/check_prometheus_rules.sh dashboards/alerts/tests/soradns_gateway_rules.test.yml`) and
  drop the stdout into the evidence bundle; this proves stale-proof/TTL/drift/TLS thresholds are
  wired correctly before the workshop.
- Capture the PromQL drill output (or promtool stdout) in the evidence bundle before moving to
  the session facilitation step.

# 6. Session Facilitation & Evidence Hand-off

This section keeps the live workshop on-rails once the dry-runs above are
completed. Moderators should follow the mini-timeline, record decisions in the
minute template, and push artefacts to governance immediately after the call.

## 6.1 Moderator timeline

| Offset | Owner | Action |
|--------|-------|--------|
| T‑24 h | Program Management | Post the reminder in `#nexus-steering`, attach the latest agenda/attendance snapshot, and link the staged artefact bundle. |
| T‑2 h | Networking TL | Re-run the GAR telemetry script and append deltas to `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`. |
| T‑15 m | Ops Automation | Verify probe host health and update the run ID in `artifacts/sorafs_gateway_dns/current`. |
| Kickoff | Moderator | Share the runbook link + minute template, call out success criteria, and assign a live scribe. |
| +15 m | Docs/DevRel | Capture action items + evidence pointers directly in the minutes doc, highlighting blockers that require governance escalation. |
| EoS | All owners | Confirm artefact uploads, note outstanding TODOs, and queue follow-up tickets. |

## 6.2 Minute template

Copy the following skeleton into `docs/source/sorafs_gateway_dns_design_minutes.md`
for each session (one file per date):

```markdown
# Gateway & DNS Kickoff — YYYY-MM-DD

- **Moderator:** <name>
- **Scribe:** <name>
- **Attendees:** <list>
- **Agenda checkpoints:** <bullets>
- **Decisions:** <numbered list>
- **Action items:** <owner → due date>
- **Evidence bundle:** `artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/`
- **Open risks/questions:** <bullets>
```

Once exported to PDF for governance, store the rendered file alongside the
artefact bundle and link it from `docs/source/sorafs_gateway_dns_design_attendance.md`.

## 6.3 Evidence upload checklist

1. Upload the tarball from § 5.5 plus the rendered minutes PDF to the governance
   bucket (`s3://sora-governance/sorafs/gateway_dns/20250302/`).
2. Record the SHA-256 hashes in the minutes and in the steering issue tracker.
3. Ping the governance reviewer alias with the upload link, ACKs from Networking
   and Storage leads, and the PagerDuty screenshot.

# 7. Post-Run Actions

1. Update `docs/source/sorafs_gateway_dns_design_pre_read.md` and the agenda
   with any new risks/assumptions discovered during the rehearsal.
2. Append a row to `ops/drill-log.md` summarising the drill, tooling versions,
  and PagerDuty outcome.
3. File a `status.md` bullet under “Latest Updates” referencing the rehearsed
   artefacts so downstream teams know the kickoff collateral is ready.
4. If conformance failures occurred, open GitHub issues with the `sf-5a` label
   and list them in `docs/source/sorafs_gateway_dns_design_attendance.md`.

# 8. Command Reference

| Command | Purpose | Output |
|---------|---------|--------|
| `cargo xtask soradns-directory-release …` | Build signed RAD/directory bundle. | `artifacts/soradns_directory/<run>/` sub-tree with JSON/TO files. |
| `./scripts/telemetry/run_soradns_transparency_tail.sh …` | Capture resolver telemetry and Pushgateway metrics. | `.jsonl` log + `.prom` metrics for GAR appendix. |
| `cargo xtask sorafs-gateway-probe …` | Validate headers, TLS/ECH, GAR enforcement. | Probe JSON, summary JSON, drill log updates, optional PagerDuty payload. |
| `ci/check_sorafs_gateway_conformance.sh` | Execute SF‑5a replay/load suites. | `artifacts/sorafs_gateway_conformance/<run>/report.json` + logs. |
| `./scripts/sorafs_gateway_self_cert.sh …` | Produce Norito self-cert bundle for rehearsal gateway. | `sorafs_gateway_report.json`, `.to` attestation, signed manifest. |

Keep this runbook next to the pre-read and update it whenever tooling, alerts,
or owner assignments change; reference the Git history in kickoff minutes.

# 9. 2025-03-02 Mock Run Snapshot

- **Run ID:** `artifacts/sorafs_gateway_dns/20250302`
- **RAD + manifest:** produced via `cargo xtask soradns-directory-release`
  (see `rad/resolver-demo.norito` and
  `20251109T090535482584Z_soradns_directory/{directory.json,record.{json,to},metadata.json}`).
- **Transparency evidence:** `logs/transparency_demo.log` plus
  `telemetry/transparency.{jsonl,prom}` from
  `scripts/telemetry/run_soradns_transparency_tail.sh`.
- **Gateway probe:** `cargo xtask sorafs-gateway-probe` against the demo headers
  writes `probe_xtask/{report.json,summary.json}`. The demo headers return `503`
  intentionally, so the run reports the failure; swap in live gateway headers to
  obtain a green probe.
- **Additional probe:** `probe/{report.json,summary.txt}` contains the lightweight
  checker output used while validating the xtask fixes.
- **Release metadata:** `directory_release/**/metadata.json` captures hashes,
  signing note (`dns-gateway-kickoff`), and resolver list.
