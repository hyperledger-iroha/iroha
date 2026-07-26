---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: تقرير معايرة إشراف الذكاء الاصطناعي (2026-02)
概要: MINFO-1 の概要:
---

# تقرير معايرة إشراف الذكاء الاصطناعي - فبراير 2026

**MINFO-1** です。データセット、マニフェスト、スコアボード
في 2026-02-05، وتمت مراجعتها من مجلس الوزارة في 2026-02-10، وتم تثبيتها في DAG الحوكمة
`912044`。

## マニフェスト

- **データセット参照:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **スラッグ:** `ai-moderation-calibration-202602`
- **エントリ:** マニフェスト 480、チャンク 12,800、メタデータ 920、オーディオ 160
- **ラベルの混合:** 安全 68%、疑わしい 19%、エスカレート 13%
- **アーティファクトダイジェスト:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **配布:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

マニフェスト `docs/examples/ai_moderation_calibration_manifest_202602.json`
ハッシュとランナーの実行。

## スコアボード

最高の opset 17 を選択してください。 JSON スコアボード
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) ハッシュ、ダイジェスト、テレメトリ、
セキュリティを強化する必要があります。

| और देखेंブライエ | ECE |オーロック |精度@検疫 |リコール@エスカレート |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 安全性（ビジョン） | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B 安全性 (マルチモーダル) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
|知覚アンサンブル (知覚) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

回答: `Brier = 0.126`、`ECE = 0.034`、`AUROC = 0.982`。 كان توزيع
合格 91.2% 検疫 6.8% エスカレート 2.0%
マニフェストを作成します。バックログとバックログ
ドリフトスコア (7.1%) スコアは 20%。

## और देखें

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- 統治動議: `MINFO-2026-02-07`
- `ministry-council-seat-03` によって `2026-02-10T11:33:12Z` で署名

خزنت CI الحزمة الموقعة في `artifacts/ministry/ai_moderation/2026-02/` مع ثنائيات
モデレートランナー。スコアボード ダイジェスト マニフェスト ハッシュ スコアボード
أعلاه أثناء عمليات التدقيق والاستئناف。

## هحات المتابعة والتنبيهات

SRE をサポート Grafana をサポート
`dashboards/grafana/ministry_moderation_overview.json` وقواعد تنبيهات Prometheus في
`dashboards/alerts/ministry_moderation_rules.yml` (تغطية الاختبارات موجودة في
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`)。フォローする
摂取とドリフト、隔離と隔離
और देखें
【AIモデレーションランナー仕様】(../../ministry/ai-moderation-runner.md)。