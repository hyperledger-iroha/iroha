---
lang: ru
direction: ltr
source: docs/source/sorafs/council_schedule.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b60a5b12176a4fe7c213ae98f38732a103cde8bb656dba3be033d3d6e7748799
source_last_modified: "2026-01-03T18:07:58.363058+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Registry Council Schedule & Minutes
---

# SoraFS Registry Council Schedule & Minutes

Roadmap guardrails for SF‑2/SF‑6 require the registry council to keep its
meeting cadence, agendas, and evidence trail publicly visible. This document is
the canonical schedule reference for the Storage/Governance program and links
directly to the minute files (`docs/source/sorafs/council_minutes_*.md`) plus
the artefact buckets that back every decision.

## Cadence & Contacts

- **Frequency:** First and third Wednesday of every month at **15:00 UTC**. When
  a session collides with a governance holiday, the Secretariat publishes the
  adjusted slot here no later than the prior meeting.
- **Location:** Sora Nexus governance bridge (video) with simultaneous IRC log,
  both captured in the corresponding `artifacts/council_minutes/<date>/` folder.
- **Chair & Secretariat:** Rotating council chair; Governance Secretariat owns
  agenda circulation, evidence collection, and publication of minutes within
  48 hours.
- **Distribution list:** `council@sora.foundation` (decision makers),
  `storage-wg@hyperledger.org` (Storage Team observers),
  `sdk-program@hyperledger.org` (SDK parity notifications).

## Upcoming Sessions (Q2 2026)

| Date (UTC) | Focus & Deliverables | Pre-read / Inputs | Evidence Owner | Notes |
|------------|---------------------|-------------------|----------------|-------|
| 2026-04-08 15:00 | Promote chunker profile `0x29` and reseal manifest governance DAG entries. Decisions include approving the signed profile envelope and gating `ChunkerProfile::Digest` rollout. | `docs/source/sorafs/chunker_registry.md`<br>`docs/source/sorafs/manifest_pipeline.md`<br>`fixtures/sorafs_chunker/manifest_signatures.json` | Storage Team · Governance Secretariat | Secretariat to pin the resealed DAG under `artifacts/sorafs/chunker_registry/2026-04-08/` before the ratification vote. |
| 2026-04-22 15:00 | Quarterly provider admission appeals plus GAR telemetry baseline refresh. Deliverables: appeals slate vote, GAR runbook change log, and dispute follow-ups. | `docs/source/sorafs/provider_admission_policy.md`<br>`docs/source/sorafs/dispute_revocation_runbook.md`<br>`dashboards/grafana/sorafs_gateway_dns.json` snapshot | Governance Secretariat · Observability | Appeals evidence tarball stored under `artifacts/council_minutes/2026-04-22/appeals_bundle.zip` with blake3 digest recorded in the minutes. |
| 2026-05-06 15:00 | Capacity marketplace guardrails + PoR counter audit. Deliverables: operator guidance ACK, storage marketplace KPI annex update, and PoR deviation response playbook notes. | `docs/source/sorafs/storage_capacity_marketplace.md`<br>`docs/source/sorafs/reports/capacity_marketplace_validation.md`<br>`docs/source/sorafs/dispute_revocation_runbook.md` | Storage WG · Treasury Guild liaison | Expect follow-up workshop for PoR telemetry; capture attendee roster + questions in the minutes addendum. |

Secretariat updates this table immediately after each vote so downstream teams
can subscribe to the correct artefact folders and pre-read bundles.

## Minutes Archive

| Date | Minutes | Decision Highlights | Evidence Bucket |
|------|---------|--------------------|-----------------|
| 2025-10-29 | [`council_minutes_2025-10-29.md`](council_minutes_2025-10-29.md) | Ratified SF‑1 architecture RFC, confirmed manifest/chunker governance plumbing, and published the CI fixture enforcement evidence. | `artifacts/council_minutes/2025-10-29/` |
| 2025-03-05 | [`council_minutes_2025-03-05.md`](council_minutes_2025-03-05.md) | Logged the priority snapshot ACK bundle, Gateway/DNS rehearsal digest, and action tracker for SF‑3/SF‑6b/SF‑9. | `artifacts/council_minutes/2025-03-05/` |

For every new meeting:

1. Copy the template below to `docs/source/sorafs/council_minutes_<YYYY-MM-DD>.md`
   and localize as needed (`.ar.md`, `.es.md`, etc.).
2. Export a PDF of the filled minutes and store it alongside the evidence bundle.
3. Update the archive table and roadmap/status references with the new link.

## Minutes Template Snippet

````markdown
---
title: SoraFS Registry Council Minutes — <YYYY-MM-DD>
summary: Topics, evidence, and decisions for the <focus> session.
---

## Attendees
- Chair:
- Storage WG:
- Governance Secretariat:
- Observers:

## Agenda
1. <item>
2. <item>

## Evidence Digest
- Bundle path: `artifacts/council_minutes/<YYYY-MM-DD>/<bundle>.tar.zst`
- BLAKE3-256: `<hex>`
- Uploaded by: `<name>` at `<timestamp>`

## Decisions
1. _Decision text_

## Action Items
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| Example | Governance Secretariat | <date> | Publish minutes PDF + artefact hash |
````

## Publishing Workflow

1. **Circulate agenda:** At least 72 hours before the session, e‑mail the agenda
   (include links above) and update this document’s Upcoming Sessions table.
2. **Collect artefacts:** Store CLI logs, Grafana exports, and council votes in
   `artifacts/council_minutes/<date>/` with recorded BLAKE3 digests. Reference
   `docs/source/sorafs/dispute_revocation_runbook.md` §2 for archival steps.
3. **During the session:** Live-edit the minute file in this directory and
   capture attendance (names + roles). Use the template’s evidence table to
   record bundle hashes as they are read into the record.
4. **Post-session:** Within 48 hours, merge the minute file, add localized
   copies if required, upload the PDF, and update the archive table above.
5. **Status/roadmap updates:** Link the new minutes from `status.md` (Latest
   Updates) and annotate the relevant roadmap entries so auditors can trace the
   decision to a specific meeting.
