---
lang: mn
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9a599a71cc49432334dbf323125756fc6056414a4b8f7622d4cc69edcfbd7503
source_last_modified: "2025-12-29T18:16:35.109243+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w2-plan
title: W2 community intake plan
sidebar_label: W2 plan
description: Intake, approvals, and evidence checklist for the community preview cohort.
---

| Item | Details |
| --- | --- |
| Wave | W2 — Community reviewers |
| Target window | Q3 2025 week 1 (tentative) |
| Artefact tag (planned) | `preview-2025-06-15` |
| Tracker issue | `DOCS-SORA-Preview-W2` |

## Objectives

1. Define the community intake criteria and vetting workflow.
2. Obtain governance approval for the proposed roster and acceptable-use addendum.
3. Refresh the checksum-verified preview artefact and telemetry bundle for the new window.
4. Stage the Try it proxy + dashboards ahead of invite dispatch.

## Task breakdown

| ID | Task | Owner | Due | Status | Notes |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Draft community intake criteria (eligibility, max slots, CoC requirements) and circulate to governance | Docs/DevRel lead | 2025‑05‑15 | ✅ Completed | Intake policy merged into `DOCS-SORA-Preview-W2` and endorsed at the 2025‑05‑20 council meeting. |
| W2-P2 | Update request template with community-specific questions (motivation, availability, localization needs) | Docs-core-01 | 2025‑05‑18 | ✅ Completed | `docs/examples/docs_preview_request_template.md` now includes the Community section, referenced in the intake form. |
| W2-P3 | Secure governance approval for the intake plan (meeting vote + recorded minutes) | Governance liaison | 2025‑05‑22 | ✅ Completed | Vote passed unanimously on 2025‑05‑20; minutes + roll call linked in `DOCS-SORA-Preview-W2`. |
| W2-P4 | Schedule Try it proxy staging + telemetry capture for the W2 window (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025‑06‑05 | ✅ Completed | Change ticket `OPS-TRYIT-188` approved and executed 2025‑06‑09 02:00–04:00 UTC; Grafana screenshots archived with ticket. |
| W2-P5 | Build/verify new preview artefact tag (`preview-2025-06-15`) and archive descriptor/checksum/probe logs | Portal TL | 2025‑06‑07 | ✅ Completed | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` ran 2025‑06‑10; outputs stored under `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Assemble community invite roster (≤25 reviewers, staged batches) with governance-approved contact info | Community manager | 2025‑06‑10 | ✅ Completed | First cohort of 8 community reviewers approved; request IDs `DOCS-SORA-Preview-REQ-C01…C08` logged in the tracker. |

## Evidence checklist

- [x] Governance approval record (meeting notes + vote link) attached to `DOCS-SORA-Preview-W2`.
- [x] Updated request template committed under `docs/examples/`.
- [x] `preview-2025-06-15` descriptor, checksum log, probe output, link report, and Try it proxy transcript stored under `artifacts/docs_preview/W2/`.
- [x] Grafana screenshots (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) captured for the W2 preflight window.
- [x] Invite roster table with reviewer IDs, request tickets, and approval timestamps populated before dispatch (see tracker W2 section).

Keep this plan updated; the tracker references it so the DOCS-SORA roadmap can see exactly what remains before W2 invitations go out.
