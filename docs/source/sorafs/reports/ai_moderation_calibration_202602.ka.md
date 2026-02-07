---
lang: ka
direction: ltr
source: docs/source/sorafs/reports/ai_moderation_calibration_202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ae480882b49c7f844742680adb4ff560231ea6790aea2fca1d1637e00969f84
source_last_modified: "2025-12-29T18:16:36.124086+00:00"
translation_last_reviewed: 2026-02-07
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
---

# AI Moderation Calibration Report - February 2026

This report packages the inaugural calibration artefacts for **MINFO-1**. The
dataset, manifest, and scoreboard were produced on 2026-02-05, reviewed by the
Ministry council on 2026-02-10, and anchored in the governance DAG at height
`912044`.

## Dataset Manifest

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

The full manifest lives in `docs/examples/ai_moderation_calibration_manifest_202602.json`
and contains the governance signature plus runner hash captured at release
time.

## Scoreboard Summary

Calibrations ran with opset 17 and the deterministic seed pipeline. The
complete scoreboard JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
records the hashes and telemetry digests; the table below highlights the most
important metrics.

| Model (family) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| -------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

Combined metrics: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. The verdict
distribution across the calibration window was pass 91.2%, quarantine 6.8%,
escalate 2.0%, matching the policy expectations recorded in the manifest
summary. False-positive backlog remained at zero, and the drift score (7.1%)
fell well below the 20% alert threshold.

## Thresholds & Sign-off

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

CI stored the signed bundle in `artifacts/ministry/ai_moderation/2026-02/`
alongside the moderation runner binaries. The manifest digest and scoreboard
hashes above must be referenced during audits and appeals.

## Dashboards & Alerts

Moderation SREs should import the Grafana dashboard at
`dashboards/grafana/ministry_moderation_overview.json` and the matching
Prometheus alert rules in `dashboards/alerts/ministry_moderation_rules.yml`
(test coverage lives under `dashboards/alerts/tests/ministry_moderation_rules.test.yml`).
These artifacts emit alerts for ingest stalls, drift spikes, and quarantine
queue growth, satisfying the monitoring requirements called out in
`docs/source/sorafs_ai_moderation_plan.md`.
