---
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 51971e1dc4e763ac7017f76c7239eef943bc21151e49e827988b61972fa58245
source_last_modified: "2025-12-29T18:16:35.107308+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-log
title: W1 feedback & telemetry log
sidebar_label: W1 feedback log
description: Aggregate roster, telemetry checkpoints, and reviewer notes for the first partner preview wave.
---

This log keeps the invite roster, telemetry checkpoints, and reviewer feedback for the
**W1 partner preview** that accompanies the acceptance tasks in
[`preview-feedback/w1/plan.md`](./plan.md) and the wave tracker entry in
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Update it whenever an invite
is sent, telemetry snapshot recorded, or feedback item triaged so governance reviewers can replay
the evidence without chasing external tickets.

## Cohort roster

| Partner ID | Request ticket | NDA received | Invite sent (UTC) | Ack/first login (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Completed 2025-04-26 | sorafs-op-01; focused on orchestrator doc parity evidence. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Completed 2025-04-26 | sorafs-op-02; validated Norito/telemetry cross-links. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Completed 2025-04-26 | sorafs-op-03; ran multi-source failover drills. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Completed 2025-04-26 | torii-int-01; Torii `/v1/pipeline` + Try it cookbook review. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Completed 2025-04-26 | torii-int-02; paired on Try it screenshot update (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Completed 2025-04-26 | sdk-partner-01; JS/Swift cookbook feedback + ISO bridge sanity checks. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Completed 2025-04-26 | sdk-partner-02; compliance cleared 2025-04-11, focused on Connect/telemetry notes. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Completed 2025-04-26 | gateway-ops-01; audited gateway ops guide + anonymised Try it proxy flow. |

Populate the **Invite sent** and **Ack** timestamps as soon as the outbound email is issued.
Anchor the times to the UTC schedule defined in the W1 plan.

## Telemetry checkpoints

| Timestamp (UTC) | Dashboards / probes | Owner | Result | Artefact |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ All green | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` transcript | Ops | ✅ Staged | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`, `probe:portal` | Docs/DevRel + Ops | ✅ Pre-invite snapshot, no regressions | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Dashboards above + Try it proxy latency diff | Docs/DevRel lead | ✅ Midpoint check passed (0 alerts; Try it latency p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Dashboards above + exit probe | Docs/DevRel + Governance liaison | ✅ Exit snapshot, zero outstanding alerts | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Daily office-hour samples (2025-04-13 → 2025-04-25) are bundled as NDJSON + PNG exports under
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` with filenames
`docs-preview-integrity-<date>.json` and corresponding screenshots.

## Feedback & issue log

Use this table to summarise reviewer-submitted findings. Link each entry to the GitHub/discuss
ticket plus the structured form captured via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Reference | Severity | Owner | Status | Notes |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Low | Docs-core-02 | ✅ Resolved 2025-04-18 | Clarified Try it nav wording + sidebar anchor (`docs/source/sorafs/tryit.md` updated with new label). |
| `docs-preview/w1 #2` | Low | Docs-core-03 | ✅ Resolved 2025-04-19 | Refreshed Try it screenshot + caption per reviewer request; artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| — | Info | Docs/DevRel lead | 🟢 Closed | Remaining comments were Q&A-only; captured in each partner’s feedback form under `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Knowledge check & survey tracking

1. Record quiz scores (target ≥90 %) for every reviewer; attach the exported CSV alongside the
   invite artefacts.
2. Collect the qualitative survey answers captured with the feedback form template and mirror them
   under `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Schedule remediation calls for anyone scoring below threshold and log them in this file.

All eight reviewers scored ≥94 % on the knowledge check (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). No remediation calls
were required; survey exports for each partner live under
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Artefact inventory

- Preview descriptor/checksum bundle: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Probe + link-check summary: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Try it proxy change log: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Telemetry exports: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Daily office-hour telemetry bundle: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Feedback + survey exports: place reviewer-specific folders under
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Knowledge check CSV and summary: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Keep the inventory in sync with the tracker issue. Attach hashes when copying artefacts to the
governance ticket so auditors can verify the files without shell access.
