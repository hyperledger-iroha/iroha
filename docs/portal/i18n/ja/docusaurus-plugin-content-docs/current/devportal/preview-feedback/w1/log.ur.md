---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プレビュー-フィードバック-w1-log
タイトル: W1 فيڈبیک اور ٹیلیمیٹری لاگ
サイドバーラベル: W1 ラベル
説明: プレビュー ウェーブ پہلی مجموعی 名簿 チェックポイント チェックポイント レビュー担当者 نوٹس۔
---

**W1 プレビュー** 招待名簿 チェックポイント レビュー担当者のフィードバック حفوظ کرتا ہے
[`preview-feedback/w1/plan.md`](./plan.md) 承認タスク ウェーブ トラッカー
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md) ٩ے ساتھ وابستہ ہیں۔フォローする
スナップショット フィードバック項目トリアージ ガバナンスレビュー担当者
بغیر بیرونی ٹکٹس کے پیچھے گئے ثبوت リプレイ کر سکیں۔

## コホート名簿

|パートナー ID |チケットのリクエスト | NDA を締結 |招待を送信しました (UTC) |確認応答/初回ログイン (UTC) |ステータス |メモ |
| --- | --- | --- | --- | --- | --- | --- |
|パートナー-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ مکمل 2025-04-26 | sorafs-op-01;オーケストレーター ドキュメントのパリティ証拠 پر فوکس۔ |
|パートナー-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ مکمل 2025-04-26 | sorafs-op-02; Norito/テレメトリクロスリンク|
|パートナー-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ مکمل 2025-04-26 | sorafs-op-03;マルチソースフェイルオーバー訓練|
|パートナー-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ مکمل 2025-04-26 |鳥居-int-01; Torii `/v2/pipeline` + Try it クックブックのレビュー۔ |
|パートナー-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ مکمل 2025-04-26 |鳥居-int-02;スクリーンショットを更新してみてください (docs-preview/w1 #2)。 |
|パートナー-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ مکمل 2025-04-26 | SDK-パートナー-01; JS/Swift クックブックのフィードバック + ISO ブリッジの健全性チェック|
|パートナー-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ مکمل 2025-04-26 | SDK-パートナー-02;コンプライアンス 2025-04-11 接続/テレメトリのメモをクリアする|
|パートナー-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ مکمل 2025-04-26 |ゲートウェイ-ops-01;ゲートウェイ運用ガイドの監査 + 匿名化された Try it プロキシ フロー|

**招待送信** اور **Ack** کے タイムスタンプ فوری طور پر درج کریں جب 送信メール جاری ہو۔
W1 پلان میں دی گئی UTC スケジュール کے مطابق رکھیں۔

## テレメトリ チェックポイント|タイムスタンプ (UTC) |ダッシュボード/プローブ |オーナー |結果 |アーティファクト |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` |ドキュメント/DevRel + オペレーション | ✅ オールグリーン | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` トランスクリプト |作戦 | ✅ ステージング | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 |ダッシュボード + `probe:portal` |ドキュメント/DevRel + オペレーション | ✅ 招待前のスナップショット、回帰なし | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 |ダッシュボード + Try it プロキシ レイテンシの差 |ドキュメント/DevRel リード | ✅ 中間点チェックに合格しました (アラート 0 件、試用レイテンシ p95=410 ミリ秒) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 |ダッシュボード + 終了プローブ |ドキュメント/DevRel + ガバナンス連携 | ✅ スナップショットを終了、未処理のアラートはゼロ | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

オフィスアワーのサンプル (2025-04-13 -> 2025-04-25) NDJSON + PNG エクスポート
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` میں موجود ہیں، فائل نام
`docs-preview-integrity-<date>.json` のスクリーンショット

## フィードバックの問題ログ

レビュー担当者の調査結果を確認するGitHub/ディスカッションへのエントリー
チケット اور اس 構造化形式 سے جوڑیں جو
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) ٩ے ذریعے بھرا گیا۔

|参考資料 |重大度 |オーナー |ステータス |メモ |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` |低い |ドキュメント-コア-02 | ✅ 2025 年 4 月 18 日に解決済み |ナビゲーションの文言 + サイドバー アンカー واضح کیا (`docs/source/sorafs/tryit.md` نئے ラベル کے ساتھ اپ ڈیٹ) をお試しください。 |
| `docs-preview/w1 #2` |低い |ドキュメントコア-03 | ✅ 2025 年 4 月 19 日に解決済み |スクリーンショット + キャプション レビュアー کی درخواست پر اپ ڈیٹ؛アーティファクト `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`。 |
| - |情報 |ドキュメント/DevRel リード | 🟢 定休日 | Q&A の質問フィードバック フォーム میں محفوظ ہیں `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`。 |

## 知識チェックとアンケートの追跡

1. レビュアーのクイズのスコア (目標 >=90%);エクスポートされた CSV にアーティファクトを招待し、添付する
2. フィードバック フォーム 定性調査の回答 フィードバック フォーム
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` ٩ے تحت ミラー کریں۔
3. しきい値の設定 修復コールのスケジュール スケジュール スケジュール ログ ログ

レビュー担当者による知識チェック میں >=94% اسکور کیا (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`)。修復コール درکار نہیں ہوئیں؛
調査輸出額:
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`。

## アーティファクトの在庫

- プレビュー記述子/チェックサム バンドル: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- プローブ + リンクチェックの概要: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- プロキシ変更ログを試してください: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- テレメトリのエクスポート: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- 毎日のオフィスアワーテレメトリーバンドル: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- フィードバック + アンケートのエクスポート: レビュー担当者固有のフォルダー
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` いいえ
- 知識チェック CSV 概要: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

在庫トラッカーの問題に関する同期の問題アーティファクト、ガバナンス チケット、ハッシュを添付する
監査人 シェル アクセス 検証 監査