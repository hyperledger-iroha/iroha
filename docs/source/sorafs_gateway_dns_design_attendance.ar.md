---
lang: ar
direction: rtl
source: docs/source/sorafs_gateway_dns_design_attendance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20e4dbd95067574ead4e1e9afc8875739e83813b2ff6b7b1e4850655906021c5
source_last_modified: "2026-01-03T18:08:00.652776+00:00"
translation_last_reviewed: 2026-01-30
---

# Attendance Tracker

Meeting: **SoraFS Gateway & DNS Design Kickoff** (2025-03-03 @ 16:00 UTC)

This tracker is archived. The kickoff completed on 2025-03-03; decisions and
follow-up ownership are recorded in
`docs/source/sorafs_gateway_dns_design_minutes.md`.

## Status Summary

- Invitations sent 2025-02-21 using `docs/source/sorafs_gateway_dns_design_invite.txt`.
- Responses were requested by **2025-02-26** and are closed.
- All confirmations received by **2025-02-26** and logged here so the workstream enters the kickoff with a closed RSVP list.

## Respondent Log

| Role | Contact | Status | Notes / Follow-up |
|------|---------|--------|-------------------|
| Networking TL (facilitator) | `networking.tl@soranet` | ✅ Confirmed 2025-02-21 | Owned the agenda and opened the bridge 10 min early. |
| Ops Lead | `ops.lead@soranet` | ✅ Confirmed 2025-02-23 | Covered rollout runbook; requested preread deck. |
| Storage Team Rep | `storage.rep@sorafs` | ✅ Confirmed 2025-02-21 | Brought latest chunker fixture status. |
| Tooling WG Rep | `tooling.wg@sorafs` | ✅ Confirmed 2025-02-21 | Prepared conformance harness updates. |
| Governance Liaison | `governance@sora` | ✅ Confirmed (delegate) 2025-02-24 | Delegate `governance.alt@sora`; primary remains OOO. |
| QA Guild Lead | `qa.guild@sorafs` | ✅ Confirmed 2025-02-21 | Received telemetry snapshot ahead of meeting. |
| Docs/DevRel Observer | `docs.devrel@sora` | ✅ Confirmed 2025-02-21 | Published minutes and outcome brief. |
| Torii Platform Rep | `torii.platform@soranet` | ✅ Confirmed 2025-02-21 | Collected GAR metrics export. |
| Security Engineering Observer | `security@soranet` | ✅ Confirmed 2025-02-25 | Slide deck shared 2025-02-24; attending remotely. |

## DNS Automation Owners

Roadmap item **Decentralized DNS & Gateway** calls for named automation owners ahead of the kickoff. The table below records the scope, accountable lead, and backup so follow-up tasks map directly to code/assets already in the repository.

| Scope | Primary Owner | Backup | Responsibilities |
|-------|---------------|--------|------------------|
| SoraDNS zonefile automation & GAR pinning | Ops Lead (`ops.lead@soranet`) | Networking TL (`networking.tl@soranet`) | Maintain the `tools/soradns-resolver/` automation, publish the signed zonefile skeletons documented in `docs/source/sns/governance_playbook.md`, and rotate SPKI/GAR data before every cutover. |
| Gateway alias & SoraFS cutover metadata | Tooling WG Rep (`tooling.wg@sorafs`) | Docs/DevRel (`docs.devrel@sora`) | Operate `docs/portal/scripts/sorafs-pin-release.sh` + `docs/portal/scripts/generate-dns-cutover-plan.mjs` (and its tests under `docs/portal/scripts/__tests__/dns-cutover-plan.test.mjs`), attach the generated manifests to `docs/portal/docs/devportal/deploy-guide.md`, and broadcast alias changes to the pin registry. |
| Telemetry snapshots & rollback automation | QA Guild Lead (`qa.guild@sorafs`) | Security Engineering (`security@soranet`) | Collect and archive GAR metrics (`docs/source/sorafs_gateway_dns_design_gar_telemetry.md`, `docs/source/sorafs_gateway_dns_design_metrics_*.prom`), ensure alert hooks stay wired into the kickoff dashboard, and rehearse the rollback flow alongside the security observer ahead of GA. |

## Follow-up Actions

1. **Deck distribution** — email final slide deck to all attendees by 2025-02-27 (Networking TL).
2. **Delegate brief** — governance delegate received the DNS policy appendix during pre-work (Docs/DevRel).
3. **Session recording logistics** — Docs/DevRel prepared the shared notes doc and recording checklist.
4. **Automation owner sync** — owners listed above confirmed runbook checkpoints (zonefile publish, alias promotion, telemetry sampling) by 2025-03-04 so the kickoff closed with clear accountability.
5. **Owner runbook** — The shared SOP now lives in `docs/source/sorafs_gateway_dns_owner_runbook.md`; it was circulated with the kickoff deck and referenced when tracking rehearsal status.
6. **Close-out confirmation** — All follow-ups above were completed and logged in `docs/source/sorafs_gateway_dns_design_minutes.md` under the 2025-03-04 close-out entry.

## Calendar & Artefacts

- Calendar invite dispatched 2025-02-25 via Google Calendar. Event link:
  `https://calendar.google.com/calendar/event?eid=c29yYWZzLWdhdGV3YXktZG5zLTIwMjUwMzAz` (SoraNet internal).
- Agenda archive: `docs/source/sorafs_gateway_dns_design_agenda.md`.
- Minutes and action register: `docs/source/sorafs_gateway_dns_design_minutes.md`.
