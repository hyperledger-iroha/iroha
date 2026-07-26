---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Rapport d'étalonnage de la modération de l'IA (2026-02)
résumé : MINFO-1 sur la version de gouvernance et sur l'ensemble de données d'étalonnage de référence, les seuils et le tableau de bord
---

# Rapport d'étalonnage de modération de l'IA - Janvier 2026

یہ رپورٹ **MINFO-1** کے لئے ابتدائی artefacts d'étalonnage کو پیک کرتی ہے۔ ensemble de données, manifeste et tableau de bord
2026-02-05 کو تیار کیے گئے، 2026-02-10 کو Conseil ministériel نے ریویو کیا، اور gouvernance DAG میں hauteur
`912044` pour ancre کیے گئے۔

## Manifeste de l'ensemble de données

- **Référence de l'ensemble de données :** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Limace :** `ai-moderation-calibration-202602`
- **Entrées :** manifeste 480, bloc 12 800, métadonnées 920, audio 160
- **Mélange d'étiquettes :** sûr 68 %, suspect 19 %, escalade 13 %
- **Résumé d'artefacts :** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution :** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

مکمل manifeste `docs/examples/ai_moderation_calibration_manifest_202602.json` میں موجود ہے
Il s'agit d'une signature de gouvernance et d'une publication et d'un hachage de coureur capturé.

## Résumé du tableau de bord

Calibrations opset 17 pour le pipeline de semences déterministes Tableau de bord JSON
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hachages et résumés de télémétrie ریکارڈ کرتا ہے؛
Voici le tableau des métriques et les détails

| Modèle (famille) | Brier | CEE | AUROC | Précision@Quarantaine | Rappel@Escalade |
| ------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Sécurité (vision) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Sécurité (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Ensemble perceptuel (perceptuel) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métriques combinées : `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Fenêtre d'étalonnage et distribution du verdict
passer 91,2%, quarantaine 6,8%, augmenter 2,0% تھا، جو résumé du manifeste میں درج attentes politiques et match کرتا ہے۔
Arriéré de faux positifs par rapport au score de dérive (7,1 %) Seuil d'alerte de 20 %

## Seuils et approbation

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Motion de gouvernance : `MINFO-2026-02-07`
- Signé par `ministry-council-seat-03` à `2026-02-10T11:33:12Z`

CI a signé un bundle signé `artifacts/ministry/ai_moderation/2026-02/` pour les binaires de modération runner et les fichiers binaires
Il s'agit d'un résumé du manifeste, de hachages du tableau de bord, d'audits, d'appels, de renvois, de hachages du tableau de bord.

## Tableaux de bord et alertes

SRE de modération et tableau de bord Grafana
Règles d'alerte `dashboards/grafana/ministry_moderation_overview.json` et Prometheus
`dashboards/alerts/ministry_moderation_rules.yml` امپورٹ کرنے چاہئیں
(couverture de test `dashboards/alerts/tests/ministry_moderation_rules.test.yml` میں ہے)۔ یہ artefacts
ingérer des stalles, des pics de dérive et une file d'attente de quarantaine et une croissance et des alertes émettent des alertes.
[Spécification de AI Moderation Runner](../../ministry/ai-moderation-runner.md) Surveillance de la surveillance des utilisateurs
exigences پوری کرتے ہیں۔