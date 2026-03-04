---
lang: uz
direction: ltr
source: docs/source/sorafs/priority_snapshot_2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0bacc9e157599e0abb78346e5300d16f6062ddbad81207e55097698940fb9b14
source_last_modified: "2025-12-29T18:16:36.121888+00:00"
translation_last_reviewed: 2026-02-07
title: SORA Nexus Priority Snapshot — 2025-03 Wave
summary: Cross-team snapshot circulated to keep the docs/content network and SoraFS storage workstreams aligned for the first March governance sync.
---

# Why this snapshot exists
- Capture the exact doc/content-network priorities requested by the Nexus steering group ahead of the March governance session.
- Provide a single link referenced by `roadmap.md` (Near-Term Execution table) so acknowledgements from Networking, Storage, and Docs leads stay traceable.
- Tie the documentation pushes to the SoraFS deliverables (SF‑3, SF‑6b, SF‑9) that remain in flight.

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
| **SF‑3 — `sorafs-node`** | Connect the PoR ingestion plumbing to `PorCoordinatorRuntime` and expose the storage proof ingestion API surface. | Plan section “Remaining integration tasks” added to `docs/source/sorafs/sorafs_node_plan.md`. | Implement the PoR ingestion worker + Norito status endpoint, then update roadmap checklist. |
| **SF‑6b — CLI/SDK polish** | Align the orchestrator bindings across Rust/JS/Swift, including retries/errors surfaced in CLI help + TypeScript definitions. | Binding polish checklist landed in `docs/source/sorafs_orchestrator_plan.md`. | Track downstream SDK PRs and record parity status in the roadmap checklist. |
| **SF‑9 — PoR coordinator runtime integration** | Thread `PorCoordinatorRuntime` into the Torii runtime loop, publish the runtime wiring plan, and document Norito events for GovernanceLog. | `docs/source/sorafs_por_plan.md` now ships an “Operational integration” section covering runtime hooks, storage, and alerts. | Implement the runtime wiring and update the roadmap SF‑9 checklist (two boxes remain). |

# Distribution checklist
- [x] Post snapshot permalink into `#nexus-steering` with summary bullets for each thread. *(Copy/paste helper: `docs/examples/nexus_steering_snapshot_post_2025-03.md`; ACKs still pending.)*
- [ ] Capture ACKs from Networking, Storage, Docs leads and update `roadmap.md` (Near-Term Execution).
- [ ] Mirror this file into the docs portal after sign-off (tracked under DocOps ticket `PORTAL-218`).

## Distribution log

| Reviewer | Role | Status | Notes |
|----------|------|--------|-------|
| @networking-tl | Networking TL | ⏳ Pending | Awaiting ✅ acknowledgment in `#nexus-steering` thread (link to paste into council minutes). |
| @storage-tl | Storage TL | ⏳ Pending | Requires confirmation that SF‑3/SF‑9 items are reflected in sprint board; add permalink once ACK lands. |
| @docs-devrel | Docs/DevRel Lead | ⏳ Pending | Needs to confirm runbook index + preview exposure plan; add screenshot/permalink to `docs/source/sorafs/council_minutes_2025-03-05.md`. |

> Reminder: capture final ACK evidence (permalinks, screenshots) inside the
> council minutes file before the March governance session.
