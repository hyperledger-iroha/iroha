---
lang: my
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1e6e4dda03f047326084118775695b76067c683eaf382388147a87518e45691e
source_last_modified: "2025-12-29T18:16:35.112223+00:00"
translation_last_reviewed: 2026-02-07
id: preview-invite-flow
title: Preview invite flow
sidebar_label: Preview invite flow
description: Sequencing, evidence, and communications plan for the docs portal public preview waves.
---

## Purpose

Roadmap item **DOCS-SORA** calls out reviewer onboarding and the public preview
invite program as the final blockers before the portal can exit beta. This page
describes how to open each invite wave, which artefacts must ship before
invites go out, and how to prove the flow is auditable. Use it alongside:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) for
  per-reviewer handling.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) for checksum
  guarantees.
- [`devportal/observability`](./observability.md) for telemetry exports and
  alerting hooks.

## Wave plan

| Wave | Audience | Entry criteria | Exit criteria | Notes |
| --- | --- | --- | --- | --- |
| **W0 – Core maintainers** | Docs/SDK maintainers validating day-one content. | `docs-portal-preview` GitHub team populated, `npm run serve` checksum gate green, Alertmanager quiet for 7 days. | All P0 docs reviewed, backlog tagged, no blocking incidents. | Used to validate the flow; no invite email, just share the preview artefacts. |
| **W1 – Partners** | SoraFS operators, Torii integrators, governance reviewers under NDA. | W0 exited, legal terms approved, Try-it proxy staged. | Collected partner sign-off (issue or signed form), telemetry shows ≤10 concurrent reviewers, no security regressions for 14 days. | Enforce invite template + request tickets. |
| **W2 – Community** | Selected contributors from the community waitlist. | W1 exited, incident drills rehearsed, public FAQ updated. | Feedback digested, ≥2 documentation releases shipped via preview pipeline without rollback. | Cap concurrent invites (≤25) and batch weekly. |

Document which wave is active inside `status.md` and in the preview request
tracker so governance can see where the program sits at a glance.

## Preflight checklist

Complete these actions **before** scheduling invites for a wave:

1. **CI artefacts available**
   - Latest `docs-portal-preview` + descriptor uploaded by
     `.github/workflows/docs-portal-preview.yml`.
   - SoraFS pin noted in `docs/portal/docs/devportal/deploy-guide.md`
     (cutover descriptor present).
2. **Checksum enforcement**
   - `docs/portal/scripts/serve-verified-preview.mjs` invoked through
     `npm run serve`.
   - `scripts/preview_verify.sh` instructions tested on macOS + Linux.
3. **Telemetry baseline**
   - `dashboards/grafana/docs_portal.json` shows healthy Try it traffic and
     `docs.preview.integrity` alert is green.
   - Latest `docs/portal/docs/devportal/observability.md` appendix updated with
     Grafana links.
4. **Governance artefacts**
   - Invite tracker issue ready (one issue per wave).
   - Reviewer registry template copied (see
     [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Legal- and SRE-required approvals attached to the issue.

Record preflight completion in the invite tracker before sending any mail.

## Flow steps

1. **Select candidates**
   - Pull from the waitlist spreadsheet or partner queue.
   - Ensure each candidate has a completed request template.
2. **Approve access**
   - Assign an approver to the invite tracker issue.
   - Verify prerequisites (CLA/contract, acceptable use, security brief).
3. **Send invites**
   - Fill in the
     [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
     placeholders (`<preview_tag>`, `<request_ticket>`, contacts).
   - Attach the descriptor + archive hash, Try it staging URL, and support
     channels.
   - Store the final email (or Matrix/Slack transcript) in the issue.
4. **Track onboarding**
   - Update the invite tracker with `invite_sent_at`, `expected_exit_at`, and
     status (`pending`, `active`, `complete`, `revoked`).
   - Link to the reviewer’s intake request for auditability.
5. **Monitor telemetry**
   - Watch `docs.preview.session_active` and `TryItProxyErrors` alerts.
   - File an incident if telemetry deviates from the baseline and record the
     outcome next to the invite entry.
6. **Collect feedback & exit**
   - Close invites once feedback lands or `expected_exit_at` passes.
   - Update the wave issue with a short summary (findings, incidents, next
     actions) before moving to the next cohort.

## Evidence & reporting

| Artefact | Where to store | Refresh cadence |
| --- | --- | --- |
| Invite tracker issue | `docs-portal-preview` GitHub project | Update after each invite. |
| Reviewer roster export | `docs/portal/docs/devportal/reviewer-onboarding.md` linked registry | Weekly. |
| Telemetry snapshots | `docs/source/sdk/android/readiness/dashboards/<date>/` (reuse telemetry bundle) | Per wave + after incidents. |
| Feedback digest | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (create folder per wave) | Within 5 days of wave exit. |
| Governance meeting note | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Populate before each DOCS-SORA governance sync. |

Run `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
after each batch to produce a machine-readable event digest. Attach the rendered
JSON to the wave issue so governance reviewers can confirm invite counts without
replaying the entire log.

Attach the evidence list to `status.md` whenever a wave ends so the roadmap
entry can be updated quickly.

## Rollback & pause criteria

Pause the invite flow (and notify governance) when any of the following occur:

- A Try it proxy incident that required rollback (`npm run manage:tryit-proxy`).
- Alert fatigue: >3 alert pages for preview-only endpoints within 7 days.
- Compliance gap: invite sent without signed terms or without logging the
  request template.
- Integrity risk: checksum mismatch detected by `scripts/preview_verify.sh`.

Resume only after documenting the remediation in the invite tracker and
confirming the telemetry dashboard is stable for at least 48 hours.
