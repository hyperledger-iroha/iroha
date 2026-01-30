---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5600f4c1fc18b3d951219208da8aadbea52141135b5731662e9d575798eb557
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# AI Moderation Calibration Report - فروری 2026

یہ رپورٹ **MINFO-1** کے لئے ابتدائی calibration artefacts کو پیک کرتی ہے۔ dataset، manifest اور scoreboard
2026-02-05 کو تیار کیے گئے، 2026-02-10 کو Ministry council نے ریویو کیا، اور governance DAG میں height
`912044` پر anchor کیے گئے۔

## Dataset Manifest

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

مکمل manifest `docs/examples/ai_moderation_calibration_manifest_202602.json` میں موجود ہے
اور اس میں governance signature کے ساتھ release کے وقت captured runner hash شامل ہے۔

## Scoreboard Summary

Calibrations opset 17 اور deterministic seed pipeline کے ساتھ چلائی گئیں۔ مکمل scoreboard JSON
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hashes اور telemetry digests ریکارڈ کرتا ہے؛
نیچے دی گئی table اہم metrics دکھاتی ہے۔

| Model (family) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| ------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

Combined metrics: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Calibration window میں verdict distribution
pass 91.2%, quarantine 6.8%, escalate 2.0% تھا، جو manifest summary میں درج policy expectations سے match کرتا ہے۔
False-positive backlog صفر رہا، اور drift score (7.1%) 20% alert threshold سے کافی نیچے تھا۔

## Thresholds اور sign-off

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

CI نے signed bundle کو `artifacts/ministry/ai_moderation/2026-02/` میں moderation runner binaries کے ساتھ محفوظ کیا۔
اوپر دیے گئے manifest digest اور scoreboard hashes کو audits اور appeals کے دوران refer کرنا ضروری ہے۔

## Dashboards & Alerts

Moderation SREs کو Grafana dashboard
`dashboards/grafana/ministry_moderation_overview.json` اور Prometheus alert rules
`dashboards/alerts/ministry_moderation_rules.yml` امپورٹ کرنے چاہئیں
(test coverage `dashboards/alerts/tests/ministry_moderation_rules.test.yml` میں ہے)۔ یہ artifacts
ingest stalls، drift spikes اور quarantine queue کی growth کے لئے alerts emit کرتے ہیں، اور
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md) میں بیان کردہ monitoring
requirements پوری کرتے ہیں۔
