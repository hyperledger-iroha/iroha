---
id: preview-feedback-w1-plan
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
---

| Item | Details |
| --- | --- |
| Wave | W1 — Partners & Torii integrators |
| Target window | Q2 2025 week 3 |
| Artefact tag (planned) | `preview-2025-04-12` |
| Tracker issue | `DOCS-SORA-Preview-W1` |

## Objectives

1. Secure legal + governance approvals for partner preview terms.
2. Stage the Try it proxy and telemetry snapshots used in the invite bundle.
3. Refresh the checksum-verified preview artefact and probe results.
4. Finalise the partner roster + request templates before invites are sent.

## Task breakdown

| ID | Task | Owner | Due | Status | Notes |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtain legal approval for the preview terms addendum | Docs/DevRel lead → Legal | 2025‑04‑05 | ✅ Completed | Legal ticket `DOCS-SORA-Preview-W1-Legal` signed off 2025‑04‑05; PDF attached to the tracker. |
| W1-P2 | Capture Try it proxy staging window (2025‑04‑10) and validate proxy health | Docs/DevRel + Ops | 2025‑04‑06 | ✅ Completed | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` executed 2025‑04‑06; CLI transcript + `.env.tryit-proxy.bak` archived. |
| W1-P3 | Build preview artefact (`preview-2025-04-12`), run `scripts/preview_verify.sh` + `npm run probe:portal`, archive descriptor/checksums | Portal TL | 2025‑04‑08 | ✅ Completed | Artefact + verification logs stored under `artifacts/docs_preview/W1/preview-2025-04-12/`; probe output attached to tracker. |
| W1-P4 | Review partner intake forms (`DOCS-SORA-Preview-REQ-P01…P08`), confirm contacts + NDAs | Governance liaison | 2025‑04‑07 | ✅ Completed | All eight requests approved (last two cleared 2025‑04‑11); approvals linked in tracker. |
| W1-P5 | Draft invite copy (based on `docs/examples/docs_preview_invite_template.md`), set `<preview_tag>` and `<request_ticket>` for each partner | Docs/DevRel lead | 2025‑04‑08 | ✅ Completed | Invite draft sent 2025‑04‑12 15:00 UTC alongside artefact links. |

## Preflight checklist

> Tip: run `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` to execute steps 1‑5 automatically (build, checksum verification, portal probe, link checker, and Try it proxy update). The script records a JSON log you can attach to the tracker issue.

1. `npm run build` (with `DOCS_RELEASE_TAG=preview-2025-04-12`) to regenerate `build/checksums.sha256` and `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` and archive `build/link-report.json` beside the descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (or provide the appropriate target via `--tryit-target`); commit the updated `.env.tryit-proxy` and keep the `.bak` for rollback.
6. Update the W1 tracker issue with log paths (descriptor checksum, probe output, Try it proxy change, Grafana snapshots).

## Evidence checklist

- [x] Signed legal approval (PDF or ticket link) attached to `DOCS-SORA-Preview-W1`.
- [x] Grafana screenshots for `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] `preview-2025-04-12` descriptor + checksum log stored under `artifacts/docs_preview/W1/`.
- [x] Invite roster table with `invite_sent_at` timestamps populated (see tracker W1 log).
- [x] Feedback artifacts mirrored in [`preview-feedback/w1/log.md`](./log.md) with one row per partner (updated 2025-04-26 with roster/telemetry/issue data).

Update this plan as tasks progress; the tracker references it to keep the roadmap
auditable.

## Feedback workflow

1. For each reviewer, duplicate the template in
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   fill the metadata, and store the completed copy under
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Summarise invites, telemetry checkpoints, and open issues inside the live log at
   [`preview-feedback/w1/log.md`](./log.md) so governance reviewers can replay the entire wave
   without leaving the repository.
3. When knowledge-check or survey exports arrive, attach them in the artefact path noted in the log
   and cross-link the tracker issue.
