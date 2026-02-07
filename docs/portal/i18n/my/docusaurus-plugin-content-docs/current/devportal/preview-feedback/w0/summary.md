---
id: preview-feedback-w0-summary
lang: my
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W0 midpoint feedback digest
sidebar_label: W0 feedback (midpoint)
description: Midpoint checkpoints, findings, and action items for the core-maintainer preview wave.
---

| Item | Details |
| --- | --- |
| Wave | W0 — Core maintainers |
| Digest date | 2025‑03‑27 |
| Review window | 2025‑03‑25 → 2025‑04‑08 |
| Participants | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Artefact tag | `preview-2025-03-24` |

## Highlights

1. **Checksum workflow** — All reviewers confirmed `scripts/preview_verify.sh`
   succeeded against the shared descriptor/archive pair. No manual overrides
   required.
2. **Navigation feedback** — Two minor sidebar ordering issues were filed
   (`docs-preview/w0 #1–#2`). Both are routed to Docs/DevRel and do not block the
   wave.
3. **SoraFS runbook parity** — sorafs-ops-01 requested clearer cross-links
   between `sorafs/orchestrator-ops` and `sorafs/multi-source-rollout`. Follow-up
   issue filed; to be addressed before W1.
4. **Telemetry review** — observability-01 confirmed `docs.preview.integrity`,
   `TryItProxyErrors`, and Try-it proxy logs stayed green; no alerts fired.

## Action items

| ID | Description | Owner | Status |
| --- | --- | --- | --- |
| W0-A1 | Reorder devportal sidebar entries to surface reviewer-focused docs (`preview-invite-*` group together). | Docs-core-01 | ✅ Completed — sidebar now lists the reviewer docs contiguously (`docs/portal/sidebars.js`). |
| W0-A2 | Add explicit cross-link between `sorafs/orchestrator-ops` and `sorafs/multi-source-rollout`. | Sorafs-ops-01 | ✅ Completed — each runbook now links to the other so operators see both guides during rollouts. |
| W0-A3 | Share telemetry snapshots + query bundle with governance tracker. | Observability-01 | ✅ Completed — bundle attached to `DOCS-SORA-Preview-W0`. |

## Exit summary (2025-04-08)

- All five reviewers confirmed completion, purged local builds, and exited the
  preview window; access revocations recorded in `DOCS-SORA-Preview-W0`.
- No incidents or alerts fired during the wave; telemetry dashboards stayed
  green for the full period.
- Navigation + cross-link actions (W0-A1/A2) are implemented and reflected in
  the docs above; telemetry evidence (W0-A3) is attached to the tracker.
- Evidence bundle archived: telemetry screenshots, invite acknowledgements, and
  this digest are linked from the tracker issue.

## Next steps

- Implement W0 action items before opening W1.
- Obtain legal approval and proxy staging slot, then follow the partner-wave
  preflight steps outlined in the [preview invite flow](../../preview-invite-flow.md).

_This digest is linked from the [preview invite tracker](../../preview-invite-tracker.md) to
keep the DOCS-SORA roadmap traceable._
