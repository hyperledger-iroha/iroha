---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Rapport de calibrage de modération IA (2026-02)
résumé : Dataset de calibrage de base, seuils et scoreboard pour la première release de gouvernance MINFO-1.
---

# Rapport de calibrage de modération IA - Février 2026

Ce rapport regroupe les artefacts de calibrage inauguraux pour **MINFO-1**. Le
dataset, le manifest et le scoreboard ont été produits le 2026-02-05, revus par
le conseil du ministère le 2026-02-10, et ancrés dans le DAG de gouvernance à la
hauteur `912044`.

## Manifeste du jeu de données

- **Référence de l'ensemble de données :** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Limace :** `ai-moderation-calibration-202602`
- **Entrées :** manifeste 480, bloc 12 800, métadonnées 920, audio 160
- **Mélange d'étiquettes :** sûr 68 %, suspect 19 %, escalade 13 %
- **Résumé d'artefacts :** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution :** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Le manifeste complet se trouve dans `docs/examples/ai_moderation_calibration_manifest_202602.json`
et contient la signature de gouvernance ainsi que le hash du runner capturé au
moment de la sortie.

## Résumé du tableau de bord

Les calibrages ont été tournés avec l'opset 17 et le pipeline de graines déterministes. Le
JSON complet du tableau de bord (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
consigner les hashes et digests de télémétrie; le tableau ci-dessous met en avant les
métriques les plus importantes.

| Modèle (famille) | Brier | CEE | AUROC | Précision@Quarantaine | Rappel@Escalade |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Sécurité (vision) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Sécurité (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Ensemble perceptuel (perceptuel) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métriques combinées : `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. La distribution
des verdicts sur la fenêtre de calibrage était passé 91,2%, quarantaine 6,8%,
escalate 2.0%, ce qui correspond aux attentes de politique indiquées dans le
résumé du manifeste. Le backlog de faux positifs est resté à zéro et le score de dérive
(7,1%) est resté bien en dessous du seuil d'alerte de 20%.

## Seuils et validation

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Motion de gouvernance : `MINFO-2026-02-07`
- Signé par `ministry-council-seat-03` à `2026-02-10T11:33:12Z`

CI a stocké le bundle signé dans `artifacts/ministry/ai_moderation/2026-02/`
aux côtés des binaires du modération runner. Le digest du manifest et les hashs
du tableau de bord ci-dessus doit être référencé lors des audits et des recours.

## Tableaux de bord et alertes

Les SRE de modération doivent importer le tableau de bord Grafana situé à
`dashboards/grafana/ministry_moderation_overview.json` et les règles d'alerte
Prometheus dans `dashboards/alerts/ministry_moderation_rules.yml` (la couverture
de tests se trouve dans `dashboards/alerts/tests/ministry_moderation_rules.test.yml`).
Ces artefacts émettent des alertes pour les blocages d'ingestion, les photos de dérive
et la croissance de la file quarantaine, répondant aux exigences de surveillance
mentionnées dans la [AI Moderation Runner Spécification](../../ministry/ai-moderation-runner.md).