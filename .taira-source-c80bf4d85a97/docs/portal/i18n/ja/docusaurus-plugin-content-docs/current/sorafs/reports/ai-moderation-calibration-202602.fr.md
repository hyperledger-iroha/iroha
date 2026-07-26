---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 調整と調整の関係 IA (2026-02)
概要: MINFO-1 のプレミア リリースのベースとなるデータセットとスコアボードのキャリブレーション。
---

# 調整と調整の関係 IA - 2026 年 2 月

**MINFO-1** を開始するための校正の成果物を再グループ化します。ル
データセット、マニフェストと製品のスコアボード、2026-02-05、revus par
le conseil du ministère le 2026-02-10, et ancrés dans le DAG de gouvernance à la
オート`912044`。

## データセットのマニフェスト

- **データセット参照:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **スラッグ:** `ai-moderation-calibration-202602`
- **エントリ:** マニフェスト 480、チャンク 12,800、メタデータ 920、オーディオ 160
- **ラベルの混合:** 安全 68%、疑わしい 19%、エスカレート 13%
- **アーティファクトダイジェスト:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **配布:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

`docs/examples/ai_moderation_calibration_manifest_202602.json` のマニフェスト完了設定
ランナーのキャプチャーを実行するための署名と内容を確認します。
リリースの瞬間。

## スコアボードの履歴書

最適なセット 17 の校正と粒度決定のパイプライン。ル
スコアボードの JSON コンプリート (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
テレメトリのハッシュとダイジェストを委託します。前衛的な風景を描く
重要な点。

|モデル（ファミーユ） |ブライエ | ECE |オーロック |精度@検疫 |リコール@エスカレート |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 安全性（ビジョン） | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B 安全性 (マルチモーダル) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
|知覚アンサンブル (知覚) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

メトリックの組み合わせ: `Brier = 0.126`、`ECE = 0.034`、`AUROC = 0.982`。ラ分布
校正試験の評決 91.2%、検疫 6.8%、
2.0% をエスカレートし、政治的不法行為に対応する
マニフェストの履歴書。ゼロとドリフトスコアの残りの疑似ポジティブのバックログ
(7.1%) 最高の安全性は 20%。

## セキュリティと検証

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- 統治動議: `MINFO-2026-02-07`
- `ministry-council-seat-03` によって `2026-02-10T11:33:12Z` で署名

CI は、`artifacts/ministry/ai_moderation/2026-02/` のバンドル署名をストックします。
オ・コート・デ・ビネール・デュ・モデレーションランナー。マニフェストのダイジェストとハッシュ
スコアボードは監査や記録を参照することができます。

## ダッシュボードとアラート

ダッシュボード Grafana のインポーターの管理を行う SRE
`dashboards/grafana/ministry_moderation_overview.json` および規則
Prometheus と `dashboards/alerts/ministry_moderation_rules.yml` (クーベルチュール
`dashboards/alerts/tests/ministry_moderation_rules.test.yml` によるテスト セット)。
摂取したブロックやドリフトの写真を注ぐ芸術品の安全性
ファイル隔離と監視の満足度の向上
[AI モデレーション ランナー仕様](../../ministry/ai-moderation-runner.md) についての言及があります。