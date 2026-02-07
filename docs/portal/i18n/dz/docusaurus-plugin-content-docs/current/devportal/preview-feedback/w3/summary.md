---
id: preview-feedback-w3-summary
lang: dz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W3 beta feedback & status
sidebar_label: W3 summary
description: Live digest for the 2026 beta preview wave (finance, observability, SDK, and ecosystem cohorts).
---

| Item | Details |
| --- | --- |
| Wave | W3 — Beta cohorts (finance + ops + SDK partner + ecosystem advocate) |
| Invite window | 2026‑02‑18 → 2026‑02‑28 |
| Artefact tag | `preview-20260218` |
| Tracker issue | `DOCS-SORA-Preview-W3` |
| Participants | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## Highlights

1. **End-to-end evidence pipeline.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` generates the per-wave summary (`artifacts/docs_portal_preview/preview-20260218-summary.json`), digest (`preview-20260218-digest.md`), and refreshes `docs/portal/src/data/previewFeedbackSummary.json` so governance reviewers can rely on a single command.
2. **Telemetry + governance coverage.** All four reviewers acknowledged checksum-gated access, submitted feedback, and were revoked on time; the digest references the feedback issues (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) alongside the Grafana runs collected during the wave.
3. **Portal surfacing.** The refreshed portal table now shows the closed W3 wave with latency and response-rate metrics, and the new log page below mirrors the event timeline for auditors who do not pull the raw JSON log.

## Action items

| ID | Description | Owner | Status |
| --- | --- | --- | --- |
| W3-A1 | Capture preview digest and attach to tracker. | Docs/DevRel lead | ✅ Completed 2026‑02‑28 |
| W3-A2 | Mirror invite/digest evidence into portal + roadmap/status. | Docs/DevRel lead | ✅ Completed 2026‑02‑28 |

## Exit summary (2026-02-28)

- Invites dispatched 2026‑02‑18 with acknowledgements logged minutes later; preview access revoked 2026‑02‑28 after the final telemetry check passed.
- Digest + summary captured under `artifacts/docs_portal_preview/`, with the raw log anchored by `artifacts/docs_portal_preview/feedback_log.json` for replayability.
- Issue follow-ups filed under `docs-preview/20260218` with the governance tracker `DOCS-SORA-Preview-20260218`; CSP/Try it notes routed to the observability/finance owners and linked from the digest.
- Tracker row updated to 🈴 Completed and the portal feedback table reflects the closed wave, completing the remaining DOCS-SORA beta-readiness task.
