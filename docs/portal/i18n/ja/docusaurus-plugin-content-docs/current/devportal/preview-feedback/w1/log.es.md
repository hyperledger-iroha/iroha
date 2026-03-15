---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プレビュー-フィードバック-w1-log
タイトル: フィードバックとテレメトリア W1 のログ
サイドバーラベル: ログ W1
説明: 名簿の集合体、テレメトリーのチェックポイント、およびパートナーのプレビュー時のレビュー担当者のメモ。
---

招待状の名簿、テレメトリーのチェックポイント、レビュー担当者によるフィードバックのエステ ログ マンティエン
**パートナー W1 のプレビュー** は、最終的な承認を得ることができます
[`preview-feedback/w1/plan.md`](./plan.md) 追跡者による追跡
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md)。現実的には羨望の的です、
テレメトリアのスナップショットを登録し、トリアージを行い、フィードバック アイテムを登録します。レビュー担当者がゴベルナンザ プエダンの再現者にアクセスできるようにします。
罪を犯した証拠のチケットを外部から。

## コホートの名簿

|パートナー ID |チケット・デ・ソリチュード | NDAレビダ |招待状 (UTC) | ACK/プライマーログイン (UTC) |エスタード |メモ |
| --- | --- | --- | --- | --- | --- | --- |
|パートナー-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 |完全版 2025-04-26 | sorafs-op-01;オーケストレーターのドキュメントを証拠として保存します。 |
|パートナー-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 |完全版 2025-04-26 | sorafs-op-02; Norito/telemetria の有効なクロスリンク。 |
|パートナー-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 |完全版 2025-04-26 | sorafs-op-03;マルチソースのフェイルオーバーをドリルします。 |
|パートナー-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 |完全版 2025-04-26 |鳥居-int-01;クックブックのリビジョン Torii `/v1/pipeline` + 試してみてください。 |
|パートナー-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 |完全版 2025-04-26 |鳥居-int-02;実際のスクリーンショットを確認して試してみてください (docs-preview/w1 #2)。 |
|パートナー-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 |完全版 2025-04-26 | SDK-パートナー-01;クックブック JS/Swift + ISO の健全性チェックのフィードバック。 |
|パートナー-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 |完全版 2025-04-26 | SDK-パートナー-02; 2025 年 4 月 11 日の準拠、接続/テレメトリアに関する注意事項。 |
|パートナー-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 |完全版 2025-04-26 |ゲートウェイ-ops-01; Audito la guia de ops del ゲートウェイ + Flujo anonimo del proxy 試してみてください。 |

**招待状**と**承認**のタイムスタンプを電子メールで送信してください。
計画 W1 でのカレンダーは UTC で定義されています。

## テレメトリのチェックポイント|タイムスタンプ (UTC) |ダッシュボード/プローブ |責任者 |結果 |アーティファクト |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` |ドキュメント/DevRel + オペレーション | Todo en verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` のトランスクリプト |作戦 |プレパラド | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 |ダッシュボード + `probe:portal` |ドキュメント/DevRel + オペレーション |招待前のスナップショット、罪の回帰 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 |ダッシュボードのアリバ + 遅延時間のプロキシの差分 試してみる |ドキュメント/DevRel リード | Chequeo de mitad de ola ok (0 アラート; 遅延 お試しください p95=410 ミリ秒) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 |アリバのダッシュボード + サリダのプローブ |ドキュメント/DevRel + ガバナンス連携 |サリダのスナップショット、保留中のセキュリティ警告 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

営業時間 (2025 年 4 月 13 日 -> 2025 年 4 月 25 日) は、NDJSON + PNG を輸出するために必要です
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` アーカイブ番号
`docs-preview-integrity-<date>.json` y はスクリーンショットの対応者を失いました。

## フィードバックの問題をログに記録する

レビューアーによる米国のタブラ パラ レスミール ホールズゴス エンビアドス。 GitHub/ディスカッションのチケットを入手する
マス・エル・フォーミュラド・キャプチャーによる構造化
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)。

|参照 |セベリダッド |責任者 |エスタード |メモ |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` |低い |ドキュメント-コア-02 |レスエルト 2025-04-18 | Try it + サイドバー (`docs/source/sorafs/tryit.md`actualizado con nuevo label) のナビゲーションの文言を確認してください。 |
| `docs-preview/w1 #2` |低い |ドキュメントコア-03 |レスエルト 2025-04-19 | Try it + キャプションの実際のスクリーンショットを確認してください。アーティファクト `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`。 |
| - |情報 |ドキュメント/DevRel リード |セラード |ロス・コメンタリオス・レスタンテス・フエロン・ソロQ&A;パートナーバジョ `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` をキャプチャし、フィードバック デ フィードバックを表示します。 |

## 知識チェックとアンケートの継続

1. Registra los puntajes del quit (objetivo >=90%) para cada reviewer;招待状を保存するための追加の CSV エクスポート。
2. フィードバックおよび参照テンプレートの調査キャプチャーを収集する
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`。
3. 管理および登録の記録に関する救済に関する議題。

Los ocho の査読者 marcaron >=94% の知識チェック (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`)。ラマダを修復する必要はありません。
ロス輸出調査パラカナダパートナー ビベン・バジョ
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`。

## 工芸品の発明品

- バンドルの記述子/チェックサムのプレビュー: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- プローブ + リンクチェックの再開: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- プロキシのログを試してみてください: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- テレメトリの輸出: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- 営業時間のテレメトリアのバンドル: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- フィードバック + アンケートのエクスポート: レビュアー bajo によるコロカル カーペット
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV および履歴書知識チェック: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`トラッカーの問題を解決するための管理。政府のチケットのコピーやアーティファクトの付加ハッシュ
シェルにアクセスしてアーカイブを検証することができます。