---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: AI モデレーション調整レポート (2026-02)
概要: MINFO-1 ガバナンス リリース ベースライン キャリブレーション データセット しきい値 スコアボード
---

# AI モデレーション調整レポート - 2026 年

**MINFO-1** ٩ے لئے ابتدائی キャリブレーション アーティファクト کو پیک کرتی ہے۔データセット、マニフェスト、スコアボード
2026-02-05 ٩و تیار کیے گئے، 2026-02-10 کو 省議会 نے ریویو کیا، اور ガバナンス DAG میں height
`912044` アンカー کیے گئے۔

## データセットマニフェスト

- **データセット参照:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **スラッグ:** `ai-moderation-calibration-202602`
- **エントリ:** マニフェスト 480、チャンク 12,800、メタデータ 920、オーディオ 160
- **ラベルの混合:** 安全 68%、疑わしい 19%、エスカレート 13%
- **アーティファクトダイジェスト:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **配布:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

マニフェスト `docs/examples/ai_moderation_calibration_manifest_202602.json` میں موجود ہے
ガバナンス署名、リリース、キャプチャされたランナーのハッシュ、

## スコアボードの概要

キャリブレーション opset 17 決定論的シード パイプラインの評価スコアボード JSON
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) ハッシュ テレメトリ ダイジェスト
دیچے دی گئی テーブル اہم metrics دکھاتی ہے۔

|モデル（ファミリー） |ブライエ | ECE |オーロック |精度@検疫 |リコール@エスカレート |
| ------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 安全性（ビジョン） | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B 安全性 (マルチモーダル) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
|知覚アンサンブル (知覚) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

結合メトリック: `Brier = 0.126`、`ECE = 0.034`、`AUROC = 0.982`。キャリブレーションウィンドウの判定分布
パス 91.2%、隔離 6.8%、エスカレート 2.0%
誤検知バックログ リスク ドリフト スコア (7.1%) 20% アラートしきい値 リスク リスク

## しきい値とサインオフ

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- 統治動議: `MINFO-2026-02-07`
- `ministry-council-seat-03` によって `2026-02-10T11:33:12Z` で署名

CI 署名付きバンドル `artifacts/ministry/ai_moderation/2026-02/` モデレーション ランナー バイナリ ساتھ محفوظ کیا۔
マニフェスト ダイジェスト スコアボード ハッシュ 監査 異議申し立て 参照 チェック マニフェスト ダイジェスト

## ダッシュボードとアラート

モデレーション SRE Grafana ダッシュボード
`dashboards/grafana/ministry_moderation_overview.json` アラート Prometheus アラート ルール
`dashboards/alerts/ministry_moderation_rules.yml` امپورٹ کرنے چاہئیں
(テストカバレッジ `dashboards/alerts/tests/ministry_moderation_rules.test.yml` میں ہے)۔アーティファクト
ストールの摂取、ドリフトスパイク、検疫キュー、成長、アラートの発生、
[AIモデレーションランナー仕様](../../ministry/ai-moderation-runner.md) モニタリング
要件 پوری کرتے ہیں۔