---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Отчет о калибровке модерации ИИ (2026-02)
résumé: Базовый калибровочный набор данных, пороги и scoreboard для первого реLISа управления MINFO-1.
---

# Отчет о калибровке модерации ИИ - Février 2026

Cela ouvre la voie aux calibrages d'artéfacts pour **MINFO-1**. Données,
manifeste et tableau de bord pour les formations 2026-02-05, рассмотрены советом
Le 10/02/2026 et l'enregistrement dans le DAG de gouvernance pour votre `912044`.

## Données du manifeste

- **Référence de l'ensemble de données :** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Limace :** `ai-moderation-calibration-202602`
- **Entrées :** manifeste 480, bloc 12 800, métadonnées 920, audio 160
- **Mélange d'étiquettes :** sûr 68 %, suspect 19 %, escalade 13 %
- **Résumé d'artefacts :** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution :** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Le manifeste complet est disponible dans `docs/examples/ai_moderation_calibration_manifest_202602.json`
et vous pouvez mettre en place un système de hash runner, ajouté à ce moment-là
реLISа.

## Tableau de bord de la Suède

Les calibrages sont effectués avec l'opset 17 et le pipeline de semences. JSON français
tableau de bord (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
содержит hachages et digère la télémétrie ; Le tableau ne contient pas de mesures clés.

| Modèle (семейство) | Brier | CEE | AUROC | Précision@Quarantaine | Rappel@Escalade |
| ----------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Sécurité (vision) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Sécurité (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Ensemble perceptuel (perceptuel) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Mesures suivantes : `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Reprise
вердиктов в окне калибровки составило passe 91,2%, quarantaine 6,8%,
augmenter de 2,0%, ce qui correspond à la politique politique du manifeste.
Le taux de dérive (7,1%) est le plus élevé.
далеко ниже порога тревоги 20%.

## Пороговые значения и согласование

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Motion de gouvernance : `MINFO-2026-02-07`
- Signé par `ministry-council-seat-03` à `2026-02-10T11:33:12Z`

CI associé au bundle de paquets dans `artifacts/ministry/ai_moderation/2026-02/`
вместе с бинарями coureur de modération. Trouver votre manifeste digest et vos hachages
le tableau d'affichage doit être utilisé pour les auditions et les apellations.

## Daschbords et alertes

SRE peut facilement importer le tableau de bord Grafana depuis
`dashboards/grafana/ministry_moderation_overview.json` et alertes Prometheus
avec `dashboards/alerts/ministry_moderation_rules.yml` (tests disponibles dans
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Эти артефACTы
Génère des alertes en cas de blocage d'ingestion, de dérive et de suivi
quarantaine, выполняя требования мониторинга, указанные в
[Spécification du coureur de modération AI] (../../ministry/ai-moderation-runner.md).