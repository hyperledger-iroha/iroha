---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: IA のカリブラソンとモデラソンの関係 (2026-02)
概要: MINFO-1 政府リリースの校正ベース、閾値、スコアボードのデータセット。
---

# IA のカリブラソン デ モデラソンの関係 - フェヴェレイロ 2026

**MINFO-1** に関する校正の技術情報を参照してください。 ○
データセット、マニフェスト、スコアボード フォーマット、製造日 2026-02-05、改訂版
conselho do Ministio em 2026-02-10 e ancorados no DAG de Governmenta na Altura
`912044`。

## マニフェスト do データセット

- **データセット参照:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **スラッグ:** `ai-moderation-calibration-202602`
- **エントリ:** マニフェスト 480、チャンク 12,800、メタデータ 920、オーディオ 160
- **ラベルの混合:** 安全 68%、疑わしい 19%、エスカレート 13%
- **アーティファクトダイジェスト:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **配布:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

O マニフェストは `docs/examples/ai_moderation_calibration_manifest_202602.json` に完了しました
私は統治の任務を遂行し、ランナーを捕らえるための瞬間を実行しなければなりません
解放する。

## Resumo do スコアボード

calibracoes の Rodaram com opset 17 は、シード決定性のパイプラインとして機能します。 ○
JSON 完全実行スコアボード (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
テレメトリのハッシュとダイジェストを登録します。メトリカスとしてのタベラ・アバイショ・デスタカ
重要なこと。

|モデロ（ファミリア） |ブライエ | ECE |オーロック |精度@検疫 |リコール@エスカレート |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 安全性（ビジョン） | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B 安全性 (マルチモーダル) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
|知覚アンサンブル (知覚) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

組み合わせの指標: `Brier = 0.126`、`ECE = 0.034`、`AUROC = 0.982`。ディストリビューション
Vereditos na Janela de calibracao foi pass 91.2%、検疫 6.8%、
2.0% をエスカレート、政治登録の期待としてアリンハダ コムは再開しない
マニフェストします。永続的な確実な確実なバックログがゼロ、ドリフト スコア (7.1%)
20% を超える可能性があります。

## しきい値とサインオフ

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- 統治動議: `MINFO-2026-02-07`
- `ministry-council-seat-03` によって `2026-02-10T11:33:12Z` で署名

CI アルマゼノウ バンドル アッシナド em `artifacts/ministry/ai_moderation/2026-02/`
junto com os binarios はモデレーションランナーを行います。 O ダイジェストはマニフェスト EOS ハッシュを行います
スコアボードは、持続的な聴覚とアペラコエスを参照することを可能にします。

## ダッシュボードとアラート

ダッシュボード Grafana の SRE 開発インポート
`dashboards/grafana/ministry_moderation_overview.json` 警告通知として送信されます
Prometheus em `dashboards/alerts/ministry_moderation_rules.yml` (コベルトゥーラ
fica em `dashboards/alerts/tests/ministry_moderation_rules.test.yml` をテストします)。エセス
アーティファトは、ストール、ドリフトスパイク、およびフィラデの摂取時にアラートを発します
検疫、監視対象の監視対象者に対する監視
【AIモデレーションランナー仕様】(../../ministry/ai-moderation-runner.md)。