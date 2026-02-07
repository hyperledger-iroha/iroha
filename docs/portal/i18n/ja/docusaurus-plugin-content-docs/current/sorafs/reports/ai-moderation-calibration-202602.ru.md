---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Отчет о калибровке модерации ИИ (2026-02)
概要: MINFO-1 のスコアボードとスコアボード。
---

# Отчет о калибровке модерации ИИ - Февраль 2026

**MINFO-1** を入手してください。だ、
マニフェストとスコアボード сформированы 2026-02-05, рассмотрены советом
2026 年 2 月 10 日、ガバナンス DAG と `912044` が一致しました。

## Манифест датасета

- **データセット参照:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **スラッグ:** `ai-moderation-calibration-202602`
- **エントリ:** マニフェスト 480、チャンク 12,800、メタデータ 920、オーディオ 160
- **ラベルの混合:** 安全 68%、疑わしい 19%、エスカレート 13%
- **アーティファクトダイジェスト:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **配布:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

`docs/examples/ai_moderation_calibration_manifest_202602.json` によるマニフェストの作成
ハッシュ ランナーを実行すると、ハッシュ ランナーが実行されます。
релиза。

## Сводка スコアボード

opset 17 とシード パイプラインをサポートします。 Полный JSON
スコアボード (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
テレメトリをハッシュし、ダイジェストします。 таблица ниже выделяет ключевые метрики.

| Модель (семейство) |ブライエ | ECE |オーロック |精度@検疫 |リコール@エスカレート |
| ----------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 安全性（ビジョン） | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B 安全性 (マルチモーダル) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
|知覚アンサンブル (知覚) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

例: `Brier = 0.126`、`ECE = 0.034`、`AUROC = 0.982`。 Распределение
合格率91.2%、検疫6.8%、
2.0% をエスカレートし、マニフェストを要求します。
Бэклог ложных срабатываний оставался нулевым、ドリフト スコア (7.1%) находился
20% です。

## Пороговые значения и согласование

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- 統治動議: `MINFO-2026-02-07`
- `ministry-council-seat-03` によって `2026-02-10T11:33:12Z` で署名

CI バンドル `artifacts/ministry/ai_moderation/2026-02/`
モデレーションランナーです。ダイジェストマニフェストとハッシュの説明
スコアボードが表示されます。

## Даборды と алерты

SRE は Grafana ダッシュボードにアクセスできます。
`dashboards/grafana/ministry_moderation_overview.json` または Prometheus
из `dashboards/alerts/ministry_moderation_rules.yml` ( тестами находится в)
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`)。 Эти артефакты
摂取、ドリフトなどの影響を及ぼします。
検疫、выполняя требования мониторинга、указанные в
【AIモデレーションランナー仕様】(../../ministry/ai-moderation-runner.md)。