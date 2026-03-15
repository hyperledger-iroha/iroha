---
lang: fr
direction: ltr
source: docs/source/sdk/swift/torii_spec_review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fb942d326b5bd031f988f44506635f804806af8fb68a63398f10173b2cf4afc5
source_last_modified: "2026-01-03T18:08:01.278533+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Swift Torii Spec Review (IOS2)

This agenda schedules the Torii spec review called out in `roadmap.md` for the
IOS2 Norito/parity milestone (`roadmap.md:1405`). The session aligns Swift,
Torii, and SDK governance owners on `/v2/pipeline` plus Norito-RPC behaviour so
the CI parity work can proceed with a frozen spec and documented action items.

## Session Overview

| Item | Details |
|------|---------|
| **Date / Time** | 2026-11-21 · 14:00–15:30 UTC |
| **Format** | Zoom (`https://meet.sora.dev/swift-torii-spec`) with collaborative notes in HackMD |
| **Audience** | Swift Program PM, Torii Platform TL, Norito tooling maintainer, SDK Program Lead, Observability TL, Android/JS parity reps, Release Engineering |
| **Goal** | Freeze the Torii `/v2/pipeline` + Norito RPC contract for IOS2, capture schema gaps that block Swift parity, and assign CI/telemetry follow-ups |
| **Recording** | Stored under `docs/source/sdk/swift/archive/meetings/2026-11-21/` (audio, chat export, and HackMD export) within 24 h |

## Objectives

1. Validate the Torii pipeline + Norito RPC spec (`docs/source/torii/nrpc_spec.md`)
   against the Swift parity blockers tracked in `docs/source/sdk/swift/connect_risk_tracker.md`
   and the weekly digest risks.
2. Confirm which proto/schema changes are landing before IOS2 freeze and align
   fixture owners for Swift/Android/JS SDKs.
3. Decide on documentation + CI updates (OpenAPI snapshots, `/v2/pipeline`
   samples, telemetry exporters) that must land before the readiness review.
4. Record owned follow-ups in `status.md` and the parity dashboard backlog with
   due dates and evidence links.

## Pre-reads & Inputs

- `docs/source/torii/nrpc_spec.md` (current Norito RPC spec)
- `docs/source/torii/norito_rpc_rollout_plan.md` + tracker (`docs/source/torii/norito_rpc_tracker.md`)
- `docs/source/status/swift_weekly_digest.md` (latest gate/risk highlights)
- `docs/source/status/swift_weekly_digest_feedback.md` (stakeholder asks)
- `docs/source/sdk/swift/connect_risk_tracker.md` (CR-2 telemetry gap context)
- `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` (fixture governance contract)
- Latest OpenAPI snapshot (`docs/portal/static/openapi/torii.json`) and checksum manifest
- Sample parity diff export (`artifacts/swift/parity_diffs/latest.json`)

## Agenda (90 minutes)

| Time (UTC) | Topic | Presenter |
|------------|-------|-----------|
| 14:00–14:10 | Welcome, objectives, success criteria | Swift Program PM |
| 14:10–14:30 | `/v2/pipeline` + Norito RPC walkthrough (request/response envelopes, retries, ABI hash policy) | Torii Platform TL |
| 14:30–14:45 | Spec delta review vs. Swift blockers (fixtures, telemetry, `/v2/pipeline` adoption) | Swift IOS2 lead |
| 14:45–15:05 | Cross-SDK fixture cadence + CI/telemetry requirements | SDK Program Lead + Observability TL |
| 15:05–15:20 | Open issues + risk register updates (CR-2/CR-3, governance dependencies) | Swift Program PM |
| 15:20–15:30 | Action capture, owners, evidence expectations | All |

## Pre-Work Checklist

- **Facilitator:** circulate the draft agenda + invite (T‑2 weeks), create the
  HackMD template with the decision/issue tables below, and ensure Zoom + recording
  permissions are set.
- **Torii Platform TL:** update the spec diff summary referencing any pending MR/PR
  IDs, attach diffs to the HackMD doc, and flag which changes require new fixtures.
- **SDK Program Lead:** export the latest parity diff artefact and parity dashboard
  screenshot; pre-populate the fixture cadence section with Tue/Fri owners.
- **Observability TL:** prepare telemetry requirements (OTLP counter list,
  dashboards impacted) and bring the backlog of `connect.queue_*` + `/v2/pipeline`
  gauges to review.
- **Release Engineering:** confirm the OpenAPI snapshot hash matches the signed
  manifest and bring the markdown snippet produced by
  `docs/portal/scripts/check-openapi-signatures.mjs`.

## Decision Capture Scaffolding

### Spec Items

| Topic | Proposed Outcome | Owner | Evidence / Due |
|-------|------------------|-------|----------------|
| `/v2/pipeline` retries + canonical errors | Freeze error catalog + retry hints for IOS2 |  |  |
| Norito RPC headers + telemetry | Confirm required headers + OTLP events |  |  |
| Fixture cadence adjustments | Tue/Fri cadence sufficient? emergency policy? |  |  |
| CI/exporter updates | What runs before merge + nightly? |  |  |

### Action Register

| ID | Description | Owner | Target | Status/Notes |
|----|-------------|-------|--------|--------------|
| TSPEC-001 | Update `docs/source/torii/nrpc_spec.md` with agreed retry hints |  | 2026-11-28 |  |
| TSPEC-002 | Regenerate Swift/Android fixtures aligned with new schema |  | 2026-11-29 |  |
| TSPEC-003 | Add `/v2/pipeline` parity tests + CI alert wiring |  | 2026-12-05 |  |
| TSPEC-004 | Update `status.md` + weekly digest with decisions |  | T+1 day |  |

## Deliverables & Follow-Ups

1. **Spec freeze memo** — update `docs/source/torii/nrpc_spec.md` with annotated
   deltas and link the meeting notes; attach HackMD export under
   `docs/source/sdk/swift/archive/meetings/2026-11-21/`.
2. **Fixture/CI commitments** — add the agreed cadence/owner notes to
   `docs/source/android_fixture_changelog.md` (if impacted) and the Swift parity
   dashboard backlog.
3. **Telemetry checklist** — extend `docs/source/status/swift_weekly_digest.md`
   + `docs/source/status/swift_weekly_digest_template.md` so `/v2/pipeline`
   adoption + telemetry readiness appear in the Governance Watchers table.
4. **Status update** — log the review completion + major outcomes in
   `status.md` under IOS2 within 24 h.
5. **Roadmap link** — reference the action IDs in `roadmap.md` once deliverables
   close so the IOS2 item reflects real evidence.

## Communication Timeline

| Time | Owner | Communication |
|------|-------|---------------|
| **T‑10 business days** | Swift Program PM | Send invite, agenda, and pre-read list via `#sdk-swift` + email |
| **T‑5 business days** | Torii Platform TL | Share spec diff summary + fixture impact note |
| **T‑2 business days** | SDK Program Lead | Post parity diff snapshot + telemetry reminders |
| **T‑1 day** | Facilitator | Confirm attendees, prep HackMD + recording |
| **T day** | Facilitator | Host session, capture live notes, assign action IDs |
| **T+1 day** | Facilitator | Publish notes/recording, update `status.md` |
| **T+1 week** | Owners | Report action progress in SDK/Torii stand-ups |

## Success Criteria

- Attendance from every listed role and recorded confirmations 24 h ahead.
- All agenda items covered within 90 minutes with captured action IDs/owners.
- Spec deltas + telemetry requirements reflected in the referenced docs within
  one week.
- `status.md` and the Swift weekly digest reference the review outcomes before
  the next governance sync.

## Logistics & Notes

- Questions ahead of the workshop should be tagged `#torii-spec` in
  `#sdk-swift` so the archive remains searchable.
- Store recordings/notes in
  `docs/source/sdk/swift/archive/meetings/2026-11-21/` for auditors; include the
  HackMD export, chat transcript, and attendee list.
- If Torii changes block the freeze, escalate via the SDK council and update the
  risk tracker (`docs/source/sdk/swift/connect_risk_tracker.md`) immediately.

_Last updated: 2026-11-17_
