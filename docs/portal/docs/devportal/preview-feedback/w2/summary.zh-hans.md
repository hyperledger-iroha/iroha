---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3d02a831f0e098972835b0b124fb8880dc825783fe28aedeaf41217620400456
source_last_modified: "2025-12-29T18:16:35.109910+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w2-summary
title: W2 community feedback & status
sidebar_label: W2 summary
description: Live digest for the community preview wave (W2).
---

| Item | Details |
| --- | --- |
| Wave | W2 — Community reviewers |
| Invite window | 2025‑06‑15 → 2025‑06‑29 |
| Artefact tag | `preview-2025-06-15` |
| Tracker issue | `DOCS-SORA-Preview-W2` |
| Participants | comm-vol-01 … comm-vol-08 |

## Highlights

1. **Governance & tooling** — Community intake policy unanimously approved on 2025‑05‑20; updated request template with motivation/timezone fields lives under `docs/examples/docs_preview_request_template.md`.
2. **Preflight evidence** — Try it proxy change `OPS-TRYIT-188` ran 2025‑06‑09, Grafana dashboards captured, and `preview-2025-06-15` descriptor/checksum/probe outputs archived under `artifacts/docs_preview/W2/`.
3. **Invite wave** — Eight community reviewers invited 2025‑06‑15, with acknowledgements logged in the tracker invite table; all completed checksum verification before browsing.
4. **Feedback** — `docs-preview/w2 #1` (tooltip wording) and `#2` (localization sidebar order) were filed on 2025‑06‑18 and resolved by 2025‑06‑21 (Docs-core-04/05); no incidents occurred during the wave.

## Action items

| ID | Description | Owner | Status |
| --- | --- | --- | --- |
| W2-A1 | Address `docs-preview/w2 #1` (tooltip wording). | Docs-core-04 | ✅ Completed 2025‑06‑21 |
| W2-A2 | Address `docs-preview/w2 #2` (localization sidebar). | Docs-core-05 | ✅ Completed 2025‑06‑21 |
| W2-A3 | Archive exit evidence + update roadmap/status. | Docs/DevRel lead | ✅ Completed 2025‑06‑29 |

## Exit summary (2025-06-29)

- All eight community reviewers confirmed completion and had preview access revoked; acknowledgements recorded in the tracker invite log.
- Final telemetry snapshots (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) remained green; logs plus Try it proxy transcripts attached to `DOCS-SORA-Preview-W2`.
- Evidence bundle (descriptor, checksum log, probe output, link report, Grafana screenshots, invite acknowledgements) archived under `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Tracker W2 checkpoint log updated through exit, ensuring the roadmap keeps an auditable record before W3 planning begins.
