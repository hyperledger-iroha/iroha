---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Rapport de calibration de modération IA (2026-02)
summary: Dataset de calibration de base, seuils et scoreboard pour la première release de gouvernance MINFO-1.
---

# Rapport de calibration de modération IA - Février 2026

Ce rapport regroupe les artefacts de calibration inauguraux pour **MINFO-1**. Le
dataset, le manifest et le scoreboard ont été produits le 2026-02-05, revus par
le conseil du ministère le 2026-02-10, et ancrés dans le DAG de gouvernance à la
hauteur `912044`.

## Manifeste du dataset

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Le manifest complet se trouve dans `docs/examples/ai_moderation_calibration_manifest_202602.json`
et contient la signature de gouvernance ainsi que le hash du runner capturé au
moment de la release.

## Résumé du scoreboard

Les calibrations ont tourné avec l'opset 17 et le pipeline de graines déterministes. Le
JSON complet du scoreboard (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
consigne les hashes et digests de telemetry; le tableau ci-dessous met en avant les
métriques les plus importantes.

| Modèle (famille) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

Métriques combinées: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. La distribution
des verdicts sur la fenêtre de calibration était pass 91.2%, quarantine 6.8%,
escalate 2.0%, ce qui correspond aux attentes de politique indiquées dans le
résumé du manifest. Le backlog de faux positifs est resté à zéro et le drift score
(7.1%) est resté bien en dessous du seuil d'alerte de 20%.

## Seuils et validation

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

CI a stocké le bundle signé dans `artifacts/ministry/ai_moderation/2026-02/`
aux côtés des binaires du moderation runner. Le digest du manifest et les hashes
du scoreboard ci-dessus doivent être référencés lors des audits et des recours.

## Dashboards et alertes

Les SRE de modération doivent importer le dashboard Grafana situé à
`dashboards/grafana/ministry_moderation_overview.json` et les règles d'alerte
Prometheus dans `dashboards/alerts/ministry_moderation_rules.yml` (la couverture
de tests se trouve dans `dashboards/alerts/tests/ministry_moderation_rules.test.yml`).
Ces artefacts émettent des alertes pour les blocages d'ingestion, les pics de drift
et la croissance de la file quarantine, satisfaisant les exigences de monitoring
mentionnées dans la [AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md).
