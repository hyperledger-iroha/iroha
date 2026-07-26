---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Relatorio de calibracao de moderacao de IA (2026-02)
résumé : Ensemble de données de base d'étalonnage, seuils et tableau de bord pour la première publication de gouvernance MINFO-1.
---

# Relatorio de calibracao de moderacao de IA - Février 2026

Ce rapport contient les artéfacts de calibrage initialisés pour **MINFO-1**. Ô
ensemble de données, le manifeste et le forum du tableau de bord produits le 2026-02-05, révisés par
Conseil du Ministère le 2026-02-10 et annoncé par le DAG de gouvernance en haute
`912044`.

## Le manifeste fait un ensemble de données

- **Référence de l'ensemble de données :** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Limace :** `ai-moderation-calibration-202602`
- **Entrées :** manifeste 480, bloc 12 800, métadonnées 920, audio 160
- **Mélange d'étiquettes :** sûr 68 %, suspect 19 %, escalade 13 %
- **Résumé d'artefacts :** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution :** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Le manifeste complet fica em `docs/examples/ai_moderation_calibration_manifest_202602.json`
e contemp a assinatura de gouvernance e o hash do runner capturado no momento do
libération.

## CV du tableau de bord

Comme calibreurs rodaram com opset 17 et le pipeline de seed deterministica. Ô
JSON complet du tableau de bord (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
enregistrer les hachages et résumés de télémétrie ; un tableau abaixo destaca comme metricas mais
importantes.

| Modèle (famille) | Brier | CEE | AUROC | Précision@Quarantaine | Rappel@Escalade |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Sécurité (vision) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Sécurité (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Ensemble perceptuel (perceptuel) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Mesures combinées : `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Un distributeur de
vereditos na janela de calibracao foi pass 91,2%, quarantaine 6,8%,
augmentation de 2,0%, alinhada com comme attentes politiques enregistrées sans curriculum vitae
manifeste. Le backlog de faux positifs permanent à zéro, et le score de dérive (7,1%)
ficou bem abaixo do limiar de alerta de 20%.

## Seuils et approbation

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Motion de gouvernance : `MINFO-2026-02-07`
- Signé par `ministry-council-seat-03` à `2026-02-10T11:33:12Z`

CI armazenou ou bundle assassiné em `artifacts/ministry/ai_moderation/2026-02/`
avec les binaires du coureur de modération. O digest se manifeste et les hachages le font
le tableau de bord doit être référencé lors des auditoires et des spectacles.

## Tableaux de bord et alertes

Les SRE de moderação doivent importer le tableau de bord Grafana dans
`dashboards/grafana/ministry_moderation_overview.json` et comme titre d'alerte
Prometheus et `dashboards/alerts/ministry_moderation_rules.yml` (couverture de
tests fica em `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Essès
les artefatos émettent des alertes pour ingérer des décrochages, des pointes de dérive et la croissance du fil de
quarantaine, en respectant les exigences de surveillance indiquées na
[Spécification du coureur de modération AI] (../../ministry/ai-moderation-runner.md).