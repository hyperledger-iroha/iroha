---
id: preview-feedback-w1-summary
lang: ka
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner feedback & exit summary
sidebar_label: W1 summary
description: Findings, actions, and exit evidence for the partner/Torii integrator preview wave.
---

| Item | Details |
| --- | --- |
| Wave | W1 — Partners & Torii integrators |
| Invite window | 2025‑04‑12 → 2025‑04‑26 |
| Artefact tag | `preview-2025-04-12` |
| Tracker issue | `DOCS-SORA-Preview-W1` |
| Participants | sorafs-op-01…03, torii-int-01…02, sdk-partner-01…02, gateway-ops-01 |

## Highlights

1. **Checksum workflow** — All reviewers verified the descriptor/archive via `scripts/preview_verify.sh`; logs stored alongside the invite acknowledgements.
2. **Telemetry** — `docs.preview.integrity`, `TryItProxyErrors`, and `DocsPortal/GatewayRefusals` dashboards stayed green for the entire wave; no incidents or alert pages fired.
3. **Doc feedback (`docs-preview/w1`)** — Two minor nits filed:
   - `docs-preview/w1 #1`: clarify nav wording in the Try it section (resolved).
   - `docs-preview/w1 #2`: update Try it screenshot (resolved).
4. **Runbook parity** — SoraFS operators confirmed the new cross-links between `orchestrator-ops` and `multi-source-rollout` addressed their W0 concerns.

## Action items

| ID | Description | Owner | Status |
| --- | --- | --- | --- |
| W1-A1 | Update Try it nav wording per `docs-preview/w1 #1`. | Docs-core-02 | ✅ Completed (2025‑04‑18). |
| W1-A2 | Refresh Try it screenshot per `docs-preview/w1 #2`. | Docs-core-03 | ✅ Completed (2025‑04‑19). |
| W1-A3 | Summarise partner findings + telemetry evidence in roadmap/status. | Docs/DevRel lead | ✅ Completed (see tracker + status.md). |

## Exit summary (2025-04-26)

- All eight reviewers confirmed completion during the final office hours, purged local artefacts, and had their access revoked.
- Telemetry remained green through exit; final snapshots attached to `DOCS-SORA-Preview-W1`.
- Invite log updated with exit acknowledgements; tracker flipped W1 to 🈴 and added the checkpoint entries.
- Evidence bundle (descriptor, checksum log, probe output, Try it proxy transcript, telemetry screenshots, feedback digest) archived under `artifacts/docs_preview/W1/`.

## Next steps

- Prepare the W2 community intake plan (governance approval + request template tweaks).
- Refresh the preview artefact tag for the W2 wave and re-run the preflight script once dates are finalised.
- Port applicable W1 findings into roadmap/status so the community wave has the latest guidance.
