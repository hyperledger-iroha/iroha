---
lang: am
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-12-29T18:16:35.071058+00:00"
translation_last_reviewed: 2026-02-07
---

# Docs preview feedback form (W1 partner wave)

Use this template when collecting feedback from W1 reviewers. Duplicate it per
partner, fill the metadata, and store the completed copy under
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.

## Reviewer metadata

- **Partner ID:** `partner-w1-XX`
- **Request ticket:** `DOCS-SORA-Preview-REQ-PXX`
- **Invite sent (UTC):** `YYYY-MM-DD hh:mm`
- **Acknowledged checksum (UTC):** `YYYY-MM-DD hh:mm`
- **Primary focus areas:** (for example _SoraFS orchestrator docs_, _Torii ISO flows_)

## Telemetry & artefact confirmations

| Checklist item | Result | Evidence |
| --- | --- | --- |
| Checksum verification | ✅ / ⚠️ | Path to log (e.g., `build/checksums.sha256`) |
| Try it proxy smoke test | ✅ / ⚠️ | `npm run manage:tryit-proxy …` transcript snippet |
| Grafana dashboard review | ✅ / ⚠️ | Screenshot path(s) |
| Portal probe report review | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

Add rows for any additional SLOs a reviewer inspects.

## Feedback log

| Area | Severity (info/minor/major/blocker) | Description | Suggested fix or question | Tracker issue |
| --- | --- | --- | --- | --- |
| | | | | |

Reference the GitHub issue or internal ticket in the last column so the preview
tracker can tie remediation items back to this form.

## Survey summary

1. **How confident are you in the checksum guidance and invite process?** (1–5)
2. **Which docs were the most/least helpful?** (short answer)
3. **Were there any blockers accessing the Try it proxy or telemetry dashboards?**
4. **Is additional localisation or accessibility content required?**
5. **Any other comments before GA?**

Capture short answers and attach raw survey exports if you use an external form.

## Knowledge check

- Score: `__/10`
- Incorrect questions (if any): `[#1, #4, …]`
- Follow-up actions (if score < 9/10): remediation call scheduled? y/n

## Sign-off

- Reviewer name & timestamp:
- Docs/DevRel reviewer & timestamp:

Store the signed copy with the associated artefacts so auditors can replay the
wave without additional context.
