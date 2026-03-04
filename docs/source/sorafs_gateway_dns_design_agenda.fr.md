---
lang: fr
direction: ltr
source: docs/source/sorafs_gateway_dns_design_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7524a4c4d40b54eb27376abafaaf2f5deedf1c7e670fbe695d729fe61b3ad41f
source_last_modified: "2026-01-03T18:07:57.685904+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Gateway & DNS Design Kickoff — Agenda
summary: Session flow, attendee roster, and decision checkpoints for the 2025-03-03 kickoff.
---

# SoraFS Gateway & DNS Design Kickoff — Agenda

**Date:** 2025-03-03  
**Time:** 16:00–17:00 UTC (60 minutes)  
**Facilitator:** Networking TL  
**Meeting type:** Decision workshop (Zoom/Meet + shared notes doc)

## 1. Attendee Roster

| Role | Name / Alias | Responsibilities |
|------|--------------|------------------|
| Networking TL (facilitator) | `networking.tl@soranet` | Drive outcome-focused discussion, capture decisions, own follow-ups. |
| Ops Lead | `ops.lead@soranet` | DNS automation, rollout runbooks, operator readiness. |
| Storage Team Rep | `storage.rep@sorafs` | Manifest integration, chunker fixtures, client orchestration impacts. |
| Tooling WG Rep | `tooling.wg@sorafs` | Conformance harness maintenance, CLI/tooling changes. |
| Governance Liaison | `governance@sora` | GAR policy alignment, escalation routing, artefact archival. |
| QA Guild Lead | `qa.guild@sorafs` | Test coverage plan, load suite resources, regression ownership. |
| Docs/DevRel Observer | `docs.devrel@sora` | Operator runbooks, Docusaurus updates, public comms. |
| Torii Platform Rep | `torii.platform@soranet` | API integration, telemetry pipelines, config surfaces. |
| Security Engineering Observer | `security@soranet` | GAR enforcement threat modelling, audit trail requirements. |

> **Action:** Confirm availability with each attendee by 2025-02-26; replace roles as needed if conflicts arise.

## 2. Pre-read Review (0–5 min)
- Quick acknowledgement that everyone read:
  - `docs/source/sorafs_gateway_dns_design_pre_read.md`
  - `docs/source/sorafs_gateway_profile.md`
  - `docs/source/sorafs_gateway_conformance.md`
  - `docs/source/sorafs_gateway_deployment_handbook.md`
- Highlight any new documents added since circulation.

## 3. Deterministic DNS Scope (5–20 min)
1. **Host Derivation Rules (5 min):**
   - Proposed canonical naming scheme (`<capability>.<lane>.gateway.sora`).
   - Ownership of namespace reservations and collision handling.
2. **Alias Proof & TTL Policy (5 min):**
   - Review SF-4a requirements, cached proof invalidation flow.
3. **Automation Path (5 min):**
   - Toolchain choice: Terraform + RFC2136 vs Torii-managed.
   - Secrets management, audit logging, GAR linkage.
4. **Decision Capture (5 min):**
   - Record final decisions in shared notes.
   - Assign owner to codify decisions in docs/config.

## 4. Gateway Enforcement & Runtime (20–40 min)
1. **GAR Policy Engine (8 min):**
   - Integration approach (library vs Norito cache).
   - Config knobs, rollout toggles.
2. **Trustless Profile Alignment (5 min):**
   - Confirm outstanding items from `sorafs_gateway_profile.md`.
3. **Direct Mode & Rate Limiting (4 min):**
   - Requirements for manifest capability enforcement, denylist hooks.
4. **Telemetry & Alerts (8 min):**
   - Metrics to capture (`torii_sorafs_gar_violations_total`, latency histograms).
   - Alert routing to governance/on-call.
5. **Decision Capture (5 min):**
   - Document acceptance criteria & owner for implementation PRs.

## 5. Conformance Harness Plan (40–50 min)
1. **Coverage Review (5 min):**
   - Replay, negative, and load suites per `sorafs_gateway_conformance.md`.
2. **Attestation Pipeline (3 min):**
   - Norito envelope format, `sorafs-gateway-cert` responsibilities.
3. **Resource & Timeline Check (2 min):**
   - QA Guild staffing, Tooling WG timelines, CI integration.

## 6. Dependencies & Roadmap Alignment (50–55 min)
- Map decisions to roadmap items (SF-4, SF-4a, SF-5, SF-5a).
- Identify blockers from SF-2/SF-3 deliverables.
- Confirm required updates to `status.md` and Docusaurus portal.

## 7. Action Register & Next Steps (55–60 min)
- Summarise decisions and outstanding actions.
- Assign owners, due dates, and follow-up checkpoints.
- Confirm publication plan for meeting notes and artefacts.

## 8. Pre-work Checklist (Owners)

| Task | Owner | Due | Notes |
|------|-------|-----|-------|
| Confirm attendee availability | Networking TL | 2025-02-26 | Use template in `docs/source/sorafs_gateway_dns_design_invite.txt`. |
| Circulate agenda & pre-read | Networking TL | 2025-02-27 | Bundle agenda + pre-read in single email. |
| Gather current GAR metric snapshot | Ops Lead / Torii Rep | 2025-02-28 | Run `scripts/telemetry/run_schema_diff.sh` variant (see action item). |
| Prepare diagrams (DNS flow, policy engine) | Tooling WG / Security | 2025-03-01 | Store under `docs/source/images/sorafs_gateway_kickoff/`. |
| Draft note-taking doc | Docs/DevRel | 2025-02-28 | Provide outline for decisions/actions. |

## 9. Post-meeting Follow-up Template
- **Within 24h:** Publish notes + action register to roadmap/status.
- **Within 48h:** Update all referenced docs with agreed decisions.
- **Within 72h:** File implementation issues/PRs and schedule progress checks.

---

For adjustments or additional agenda items, comment directly on this document or ping `networking.tl@soranet`. All changes must be frozen 24 hours before the session to keep the meeting focused on decision outcomes.
