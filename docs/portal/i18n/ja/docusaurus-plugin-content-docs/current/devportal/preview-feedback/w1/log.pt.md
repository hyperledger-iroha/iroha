---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プレビュー-フィードバック-w1-log
タイトル: フィードバックとテレメトリア W1 のログ
サイドバーラベル: ログ W1
説明: 名簿の集合体、テレメトリーのチェックポイント、およびパルセイロスのプレビューに関するレビュアーの記録。
---

招待者の名簿管理、テレメトリーのチェックポイント、レビュー担当者によるフィードバックのエステログ
**パルセイロス W1 のプレビュー** タレファス デ エースイタカオとして戦える
[`preview-feedback/w1/plan.md`](./plan.md) トラッカーのアクセス許可を取得します
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md)。 enviado のために quando um convite を実現し、
レジストラド用のテレメトリアのスナップショット、統治者ポッサムのトライアドパラケレビュー担当者用のアイテムのフィードバック
チケットを外部から証拠として再現します。

## ロスター・ダ・コート

|パートナー ID |チケット・デ・ソリシタカオ | NDA 受信 |コンバイトエンビアド (UTC) | Ack/primeiro ログイン (UTC) |ステータス |メモ |
| --- | --- | --- | --- | --- | --- | --- |
|パートナー-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 |まとめ 2025-04-26 | sorafs-op-01;オーケストレーターを実行するためのドキュメントの証拠を収集します。 |
|パートナー-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 |まとめ 2025-04-26 | sorafs-op-02; validou クロスリンク Norito/telemetria。 |
|パートナー-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 |まとめ 2025-04-26 | sorafs-op-03;マルチソースフェイルオーバーのドリルを実行します。 |
|パートナー-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 |まとめ 2025-04-26 |鳥居-int-01; revisao do 料理本 Torii `/v2/pipeline` + 試してみましょう。 |
|パートナー-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 |まとめ 2025-04-26 |鳥居-int-02;スクリーンショットを実際に試してみてください (docs-preview/w1 #2)。 |
|パートナー-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 |まとめ 2025-04-26 | SDK-パートナー-01;クックブックのフィードバック JS/Swift + サニティ チェックにより ISO ブリッジが行われます。 |
|パートナー-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 |まとめ 2025-04-26 | SDK-パートナー-02; 2025 年 4 月 11 日の法令遵守、接続/テレメトリに関する注意事項。 |
|パートナー-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 |まとめ 2025-04-26 |ゲートウェイ-ops-01; Auditou o guia ops do ゲートウェイ + fluxo anonimo do proxy 試してみてください。 |

**Convite enviado** と **Ack** のタイムスタンプを、送信用のメールで送信してください。
アンコア オス ホラリオス、クロノグラマ UTC 定義、プラノ W1 はありません。

## テレメトリのチェックポイント|タイムスタンプ (UTC) |ダッシュボード/プローブ |返信 |結果 |アルテファト |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` |ドキュメント/DevRel + オペレーション |トゥード ヴェルデ | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` のトランスクリプト |作戦 |ステージング | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 |ダッシュボード acima + `probe:portal` |ドキュメント/DevRel + オペレーション |招待前のスナップショット、SEM 回帰 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 |ダッシュボード acima + diff de latencia do proxy 試してみる |ドキュメント/DevRel リード |最初のチェックポイント (0 アラート; 遅延 お試しください p95=410 ミリ秒) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 |ダッシュボード acima + プローブ デ サイダ |ドキュメント/DevRel + ガバナンス連携 |スナップショット、保留中のアラートはゼロ | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

オフィスアワーの日記として (2025-04-13 -> 2025-04-25) estao agrupadas como exports NDJSON + PNG em
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` com の名前
`docs-preview-integrity-<date>.json` のスクリーンショットの対応者。

## フィードバックの問題をログに記録する

レビュー担当者は、esta tabela para resumir achados enviados を使用してください。 Vincule cada entrada ao ticket GitHub/ディスカッション
mais o Formulario estruturado capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)。

|参照 |セヴェリダーデ |返信 |ステータス |メモ |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` |低い |ドキュメント-コア-02 |解決策 2025-04-18 | Esclareceu の文言をナビゲートしてみてください + サイドバー (`docs/source/sorafs/tryit.md` atualizado com novo label)。 |
| `docs-preview/w1 #2` |低い |ドキュメントコア-03 |解決策 2025-04-19 | Try it + legenda atualizados conmepedido のスクリーンショット。アルテファト `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`。 |
| - |情報 |ドキュメント/DevRel リード |フェチャド | Q&A に関するコメントを掲載しています。 `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` では、パルセイロのすすり泣くようなフィードバックのキャプチャはありません。 |

## 知識チェックとアンケートの実施

1. CAD レビュアーとしてクイズ (メタ >=90%) を行わないように登録します。別の CSV エクスポートを参照してください。
2. フィードバックのテンプレートなしでアンケートのキャプチャを行う質の高い応答として収集する
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`。
3. 登録を制限するために、修復に関する議題を設定します。

Todos os oito のレビュー担当者 marcaram >=94% 知識チェックなし (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`)。ネンフマ・チャマダ・デ・レメディエーション
必要なことはありません。パルセイロの調査に関する輸出
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`。

## 芸術品の発明

- バンドル プレビュー記述子/チェックサム: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- プローブ + リンクチェックの再開: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- プロキシのログ記録: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- テレメトリのエクスポート: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- 営業時間のテレメトリアのバンドル: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- フィードバックとアンケートのエクスポート: レビュアーによるコロカルパスタ
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV e resumo do 知識チェック: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantenha はトラッカーを発行します。 Anexe は政府のチケットのコピーやアートファトをハッシュします
シェルのアクセス権を検証するためのオーディオ情報です。