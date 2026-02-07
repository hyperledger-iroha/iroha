---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 672a5e3a6f0c3e8999400bc6fa8c66cc3be1ba2119431c5fd26f6d9a436f767f
source_last_modified: "2025-12-29T18:16:35.187152+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Gateway & DNS Kickoff Runbook

This portal copy mirrors the canonical runbook in
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
It captures the operational guardrails for the Decentralized DNS & Gateway
workstream so networking, ops, and documentation leads can rehearse the
automation stack ahead of the 2025‑03 kickoff.

## Scope & Deliverables

- Bind the DNS (SF‑4) and gateway (SF‑5) milestones by rehearsing deterministic
  host derivation, resolver directory releases, TLS/GAR automation, and evidence
  capture.
- Keep the kickoff inputs (agenda, invite, attendance tracker, GAR telemetry
  snapshot) synchronized with the latest owner assignments.
- Produce an auditable artefact bundle for governance reviewers: resolver
  directory release notes, gateway probe logs, conformance harness output, and
  the Docs/DevRel summary.

## Roles & Responsibilities

| Workstream | Responsibilities | Required artefacts |
|------------|------------------|--------------------|
| Networking TL (DNS stack) | Maintain deterministic host plan, run RAD directory releases, publish resolver telemetry inputs. | `artifacts/soradns_directory/<ts>/`, diffs for `docs/source/soradns/deterministic_hosts.md`, RAD metadata. |
| Ops Automation Lead (gateway) | Execute TLS/ECH/GAR automation drills, run `sorafs-gateway-probe`, update PagerDuty hooks. | `artifacts/sorafs_gateway_probe/<ts>/`, probe JSON, `ops/drill-log.md` entries. |
| QA Guild & Tooling WG | Run `ci/check_sorafs_gateway_conformance.sh`, curate fixtures, archive Norito self-cert bundles. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | Capture minutes, update the design pre-read + appendices, and publish the evidence summary in this portal. | Updated `docs/source/sorafs_gateway_dns_design_*.md` files and rollout notes. |

## Inputs & Prerequisites

- Deterministic host spec (`docs/source/soradns/deterministic_hosts.md`) and the
  resolver attestation scaffolding (`docs/source/soradns/resolver_attestation_directory.md`).
- Gateway artefacts: operator handbook, TLS/ECH automation helpers,
  direct‑mode guidance, and self-cert workflow under `docs/source/sorafs_gateway_*`.
- Tooling: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, and CI helpers
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets: GAR release key, DNS/TLS ACME credentials, PagerDuty routing key,
  Torii auth token for resolver fetches.

## Pre-flight Checklist

1. Confirm attendees and agenda by updating
   `docs/source/sorafs_gateway_dns_design_attendance.md` and circulating the
   current agenda (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Stage artefact roots such as
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` and
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Refresh fixtures (GAR manifests, RAD proofs, gateway conformance bundles) and
   ensure `git submodule` state matches the latest rehearsal tag.
4. Verify secrets (Ed25519 release key, ACME account file, PagerDuty token) are
   present and match vault checksums.
5. Smoke-test telemetry targets (Pushgateway endpoint, GAR Grafana board) prior
   to the drill.

## Automation Rehearsal Steps

### Deterministic host map & RAD directory release

1. Run the deterministic host derivation helper against the proposed manifest
   set and confirm there is no drift from
   `docs/source/soradns/deterministic_hosts.md`.
2. Generate a resolver directory bundle:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Record the printed directory ID, SHA-256, and output paths inside
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` and the kickoff
   minutes.

### DNS telemetry capture

- Tail resolver transparency logs for ≥10 minutes using
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Export Pushgateway metrics and archive the NDJSON snapshots alongside the run
  ID directory.

### Gateway automation drills

1. Execute the TLS/ECH probe:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Run the conformance harness (`ci/check_sorafs_gateway_conformance.sh`) and
   the self-cert helper (`scripts/sorafs_gateway_self_cert.sh`) to refresh the
   Norito attestation bundle.
3. Capture PagerDuty/Webhook events to prove the automation path works end to
   end.

### Evidence packaging

- Update `ops/drill-log.md` with timestamps, participants, and probe hashes.
- Store artefacts under the run ID directories and publish an executive summary
  in the Docs/DevRel meeting minutes.
- Link the evidence bundle in the governance ticket before the kickoff review.

## Session facilitation & evidence hand-off

- **Moderator timeline:**  
  - T‑24 h — Program Management posts the reminder + agenda/attendance snapshot in `#nexus-steering`.  
  - T‑2 h — Networking TL refreshes the GAR telemetry snapshot and records deltas in `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.  
  - T‑15 m — Ops Automation verifies probe readiness and writes the active run ID into `artifacts/sorafs_gateway_dns/current`.  
  - During the call — Moderator shares this runbook and assigns a live scribe; Docs/DevRel capture action items inline.
- **Minute template:** Copy the skeleton from
  `docs/source/sorafs_gateway_dns_design_minutes.md` (also mirrored in the portal
  bundle) and commit one filled instance per session. Include attendee roll,
  decisions, action items, evidence hashes, and outstanding risks.
- **Evidence upload:** Zip the `runbook_bundle/` directory from the rehearsal,
  attach the rendered minutes PDF, record SHA-256 hashes in the minutes + agenda,
  then ping the governance reviewer alias once uploads land in
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Evidence snapshot (March 2025 kickoff)

The latest rehearsal/live artefacts referenced in the roadmap and governance
minutes live under the `s3://sora-governance/sorafs/gateway_dns/` bucket. Hashes
below mirror the canonical manifest (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Dry run — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Bundle tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Minutes PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Live workshop — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Pending upload: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel will append the SHA-256 once the rendered PDF lands in the bundle.)_

## Related Material

- [Gateway operations playbook](./operations-playbook.md)
- [SoraFS observability plan](./observability-plan.md)
- [Decentralized DNS & Gateway tracker](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
