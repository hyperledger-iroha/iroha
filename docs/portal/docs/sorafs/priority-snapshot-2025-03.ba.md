---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c11fe861e7052b113b91249eb9e39adca67a3b3cc20acf497f0785e37498504c
source_last_modified: "2025-12-29T18:16:35.196700+00:00"
translation_last_reviewed: 2026-02-07
id: priority-snapshot-2025-03
title: Priority Snapshot — March 2025 Archive
description: Archived mirror of the 2025-03 Nexus steering snapshot with implementation status separated from external rollout evidence.
translator: machine-google-reviewed
---

# Why this snapshot exists
- Capture the exact doc/content-network priorities requested by the Nexus steering group ahead of the March governance session.
- Provide a single link referenced by `roadmap.md` (Near-Term Execution table) so acknowledgements from Networking, Storage, and Docs leads stay traceable.
- Preserve the March SoraFS delivery hand-off while distinguishing shipped local
  implementation work from hosted rollout evidence that still needs operator
  archival.

# Focus threads

## 1. Circulate priority snapshot
- **Owners:** Program Management + Docs.
- **Action:** Share this file in the Nexus steering channel, collect ✅ emoji ACKs from Networking TL, Storage TL, and Docs/DevRel lead, and log screenshots/links in the governance meeting notes.
- **Deadline:** 2025‑03‑04 12:00 UTC (48 h before the steering session).
- **Evidence:** Paste the channel permalink and ACK list into `docs/source/sorafs/council_minutes_2025-03-05.md` once the session closes.

## 2. Gateway/DNS kickoff close-out
- **Owners:** Networking TL, Ops Automation lead.
- **Action:** Use the new Section 6 “Session facilitation & evidence hand-off” in `docs/source/sorafs_gateway_dns_design_runbook.md` to run the dry-run, document ownership of the minute template, and pre-fill the artefact manifest before the 2025‑03‑03 workshop.
- **Dependencies:** Updated attendees list + GAR telemetry snapshot (see `docs/source/sorafs_gateway_dns_design_*` files).

## 3. Operator runbook migration
- **Owners:** Docs/DevRel.
- **Action:** Publish the consolidated `Runbook Index` in the docs portal (`docs/portal/docs/sorafs/runbooks-index.md`) so reviewers have a single navigation anchor; mark the migration row in `roadmap.md` complete once the index is live and wired to the sidebar.
- **Follow-up:** ✅ Completed — the DocOps wave now advertises the beta preview host at `https://docs.iroha.tech/` inside the portal index so reviewers can reach the checksum-gated snapshot once onboarding closes.【docs/portal/docs/sorafs/runbooks-index.md:1】

## 4. SoraFS delivery threads

| Item | Scope | Latest action | Next blocker |
|------|-------|---------------|--------------|
| **SF‑3 — `sorafs-node`** | PoR ingestion plumbing, storage proof status, and operator replay surface. | Local PoR ingestion/status and `sorafs-node ingest por` workflows are documented in `docs/source/sorafs/sorafs_node_plan.md` and `docs/source/sorafs/runbooks/sorafs_node_ops.md`. | Archive hosted rollout evidence and SDK/operator ergonomics follow-ups. |
| **SF‑6b — CLI/SDK polish** | Align the orchestrator bindings across Rust/JS/Swift, including retries/errors surfaced in CLI help + TypeScript definitions. | Binding polish checklist landed in `docs/source/sorafs_orchestrator_plan.md`. | Track downstream SDK PRs and record parity status in the roadmap checklist. |
| **SF‑9 — PoR coordinator runtime integration** | Thread `PorCoordinatorRuntime` into the Torii runtime loop, publish runtime wiring, and document Norito events for GovernanceLog. | Local runtime wiring, verifier/exporter helpers, and governance report fixtures are reflected in `docs/source/sorafs_por_plan.md` and `roadmap.md`. | Capture live drand/VRF/auditor run evidence and governance archive hand-off. |

# Distribution checklist
- [x] Post snapshot permalink into `#nexus-steering` with summary bullets for each thread. *(Copy/paste helper: `docs/examples/nexus_steering_snapshot_post_2025-03.md`.)*
- [x] Record implementation status in `roadmap.md` and `status.md`; no repo-local
  SoraFS code item is blocked on the March ACK archive.
- [x] Keep this source snapshot as the durable archive for DocOps ticket
  `PORTAL-218`; publish portal mirrors from current runbooks instead of this
  historical hand-off note.

## Distribution log

| Reviewer | Role | Status | Notes |
|----------|------|--------|-------|
| @networking-tl | Networking TL | Archived outside repo | No blocking SoraFS implementation item remains tied to the March acknowledgment thread. |
| @storage-tl | Storage TL | Archived outside repo | SF‑3/SF‑9 local implementation status is now reflected in the roadmap and runbooks. |
| @docs-devrel | Docs/DevRel Lead | Archived outside repo | The runbook index and preview-host references are maintained in the portal docs. |

This snapshot is historical. Future changes should update the live runbooks,
`roadmap.md`, and `status.md`, while external governance ACK evidence remains
in the meeting archive rather than this repo-local implementation tracker.
