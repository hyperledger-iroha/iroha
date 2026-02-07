---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Information de calibrage de modération de IA (2026-02)
résumé : Ensemble de données de base d'étalonnage, paramètres et tableau de bord pour le premier lancement du gouvernement MINFO-1.
---

# Informe de calibrage de modération de IA - Février 2026

Este informe empaqueta los artefactos de calibración inaugurales para **MINFO-1**. El
l'ensemble de données, le manifeste et le tableau de bord seront produits le 2026-02-05, seront révisés par le
Conseil du Ministère le 2026-02-10 et anclaron au DAG de gobernanza en la altura
`912044`.

## Manifeste de l'ensemble de données

- **Référence de l'ensemble de données :** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Limace :** `ai-moderation-calibration-202602`
- **Entrées :** manifeste 480, bloc 12 800, métadonnées 920, audio 160
- **Mélange d'étiquettes :** sûr 68 %, suspect 19 %, escalade 13 %
- **Résumé d'artefacts :** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution :** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Le manifeste complet vive en `docs/examples/ai_moderation_calibration_manifest_202602.json`
et contient la société de gouvernement plus le hachage du coureur capturé au moment du
libération.

## Résumé du tableau de bord

Les calibrages sont effectués avec l'option 17 et le pipeline de semis déterministes. El
JSON complet du tableau de bord (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
enregistrer les hachages et les résumés de télémétrie ; le tableau suivant destaca les métriques plus
importantes.

| Modèle (famille) | Brier | CEE | AUROC | Précision@Quarantaine | Rappel@Escalade |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Sécurité (vision) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Sécurité (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Ensemble perceptuel (perceptuel) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métriques combinées : `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. La distribution de
les contrôles sur la ventana de calibrage fue pass 91,2%, quarantaine 6,8%,
augmentation de 2,0%, coïncidant avec les attentes politiques enregistrées en
reprendre le manifeste. L’arriéré de faux positifs se maintient en zéro et le
score de dérive (7,1%) qui était très faible en raison de l'ombre d'alerte de 20%.

## Parapluies et approbation

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Motion de gouvernance : `MINFO-2026-02-07`
- Signé par `ministry-council-seat-03` à `2026-02-10T11:33:12Z`

CI stocke le bundle ferme en `artifacts/ministry/ai_moderation/2026-02/`
avec les binaires du coureur de modération. Le résumé du manifeste et des hachages
Le tableau de bord antérieur doit être référencé lors des auditions et des apparitions.

## Tableaux de bord et alertes

Le SRE de modération doit importer le tableau de bord de Grafana fr
`dashboards/grafana/ministry_moderation_overview.json` et les règles d'alerte de
Prometheus et `dashboards/alerts/ministry_moderation_rules.yml` (la couverture des tests
vive en `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Estos
Les artefacts émettent des alertes en raison des blocages d'absorption, des pics de dérive et de l'augmentation de
la cola de quarantaine, remplissant les exigences de surveillance indiquées en la
[Spécification du coureur de modération AI] (../../ministry/ai-moderation-runner.md).