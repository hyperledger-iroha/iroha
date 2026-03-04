---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: IA の校正に関する情報 (2026-02)
概要: MINFO-1 の基礎となる校正ベース、スコアボードのデータセット。
---

# IA の調整に関する情報 - 2026 年 2 月

**MINFO-1** の開始に関する校正情報をお知らせします。エル
データセット、マニフェストとスコアボード、2026 年 2 月 5 日作成、燃料改訂版
2026 年 2 月 10 日、政府会議が行われ、DAG デ ゴベルナンザとアルトゥーラが行われます。
`912044`。

## データセットをマニフェストします

- **データセット参照:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **スラッグ:** `ai-moderation-calibration-202602`
- **エントリ:** マニフェスト 480、チャンク 12,800、メタデータ 920、オーディオ 160
- **ラベルの混合:** 安全 68%、疑わしい 19%、エスカレート 13%
- **アーティファクトダイジェスト:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **配布:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

`docs/examples/ai_moderation_calibration_manifest_202602.json` で生き続けることを宣言します
私は、ランナーの瞬間を捉えたハッシュを実行します。
解放する。

## スコアボードの履歴書

17 年間のパイプラインの最終決定に向けて、排出計画の最終校正が行われます。エル
スコアボードの完全な JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
テレメトリのハッシュとダイジェストの登録。メトリカス マスのタブラ シグエンテ デスタカ
重要なこと。

|モデロ（ファミリア） |ブライエ | ECE |オーロック |精度@検疫 |リコール@エスカレート |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 安全性（ビジョン） | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B 安全性 (マルチモーダル) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
|知覚アンサンブル (知覚) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

組み合わせの指標: `Brier = 0.126`、`ECE = 0.034`、`AUROC = 0.982`。配布先
校正パスの検証 91.2%、検疫 6.8%、
2.0% のエスカレート、政治登録の予想と一致
マニフェストを再開します。初期段階での確実な事実のバックログ
ドリフト スコア (7.1%) ケド ムイ ポル デバホ デル アンブラル デ アラータ デル 20%。

## 傘と悪用

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- 統治動議: `MINFO-2026-02-07`
- `ministry-council-seat-03` によって `2026-02-10T11:33:12Z` で署名

CI アルマセノ エル バンドル ファームド en `artifacts/ministry/ai_moderation/2026-02/`
ジュント・コン・ロス・ビナリオス・デル・モデレーションランナー。エルダイジェストデルマニフェストとロスハッシュ
スコアボードの前のデベン参照は、持続的な視聴とアペラシオンを参照します。

## ダッシュボードとアラート

Grafana ja のダッシュボードのインポートでの損失 SRE
`dashboards/grafana/ministry_moderation_overview.json` 警告に関する最新情報
Prometheus と `dashboards/alerts/ministry_moderation_rules.yml` (テストの詳細)
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`) を生きてください。エストス
アーティファクトは、摂取量のブロック、ドリフトと証拠のピコとしてアラートを発します
ラ・コーラ・デ・検疫、クンプリエンド・ロス・リクエスト・デ・モニター・インディカドス・アン・ラ
【AIモデレーションランナー仕様】(../../ministry/ai-moderation-runner.md)。