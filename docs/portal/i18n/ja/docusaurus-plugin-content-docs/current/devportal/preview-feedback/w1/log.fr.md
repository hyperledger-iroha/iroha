---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プレビュー-フィードバック-w1-log
タイトル: ジャーナルフィードバックとテレメトリ W1
サイドバーラベル: ジャーナル W1
説明: 名簿の同意、遠隔測定のチェックポイント、およびメモのレビュー担当者が、プレミアの曖昧なプレビュー パートナーに注力します。
---

Ce ジャーナルは、招待状の名簿、テレメトリーのチェックポイント、およびフィードバック査読者の情報を保存します。
**パートナー W1 のプレビュー** 受け入れられるかどうかを確認する
[`preview-feedback/w1/plan.md`](./plan.md) 曖昧な追跡者のエントリ
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md)。 Mettez-le a jour quand une Invitation est envoyee、
テレメトリのスナップショットを登録し、項目のフィードバックを確認してレビュー担当者のガバナンスを強化します
rejouer les preuves sans courir apres des ticket externes。

## コホートの名簿

|パートナー ID |チケット・デ・デマンド | NDA レク |特使を招待 (UTC) | Ack/プレミアログイン (UTC) |法令 |メモ |
| --- | --- | --- | --- | --- | --- | --- |
|パートナー-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 |テルミネ 2025-04-26 | sorafs-op-01; concentre sur les preuves de parite de docs オーケストレーター。 |
|パートナー-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 |テルミネ 2025-04-26 | sorafs-op-02;有効なクロスリンク Norito/telemetrie。 |
|パートナー-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 |テルミネ 2025-04-26 | sorafs-op-03;フェイルオーバーマルチソースのドリルを実行します。 |
|パートナー-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 |テルミネ 2025-04-26 |鳥居-int-01;レビュー ドゥ クックブック Torii `/v1/pipeline` + 試してみましょう。 |
|パートナー-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 |テルミネ 2025-04-26 |鳥居-int-02; a accompagne la mise a jour de capture 試してみてください (docs-preview/w1 #2)。 |
|パートナー-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 |テルミネ 2025-04-26 | SDK-パートナー-01;フィードバック クックブック JS/Swift + サニティ チェック ISO ブリッジ。 |
|パートナー-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 |テルミネ 2025-04-26 | SDK-パートナー-02;コンプライアンスは 2025 年 4 月 11 日有効、接続/テレメトリーに関するメモを中心にしています。 |
|パートナー-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 |テルミネ 2025-04-26 |ゲートウェイ-ops-01; Audit du guide ops ゲートウェイ + Flux プロキシ 匿名化してみてください。 |

レンセニェス **特使を招待**し、**確認**してください。メールでの送受信をお願いします。
Ancrez les heures au plan UTC 定義とプラン W1。

## チェックポイントテレメトリー|ホロダタージュ (UTC) |ダッシュボード/プローブ |責任者 |結果 |アーティファクト |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` |ドキュメント/DevRel + オペレーション |タウト・ヴェール | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 |トランスクリプト `npm run manage:tryit-proxy -- --stage preview-w1` |作戦 |ステージング | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 |ダッシュボード ci-dessus + `probe:portal` |ドキュメント/DevRel + オペレーション |招待前のスナップショット、角質回帰 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 |ダッシュボード ci-dessus + diff de latence proxy 試してみる |ドキュメント/DevRel リード |有効なチェックポイント環境 (アラート 0 件、遅延 試してみる p95=410 ミリ秒) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 |ダッシュボード ci-dessus + プローブ デ ソーティ |ドキュメント/DevRel + ガバナンス連携 |出撃時のスナップショット、再警報ゼロ | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Les echantillons quotidiens d'office hours (2025-04-13 -> 2025-04-25) エクスポートと NDJSON + PNG の再グループ化
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` フィシエの平均値
`docs-preview-integrity-<date>.json` et les は通信相手をキャプチャします。

## フィードバックと問題をログに記録する

レビュアーの統計情報を履歴書に表示して活用します。 Liez チャク エントレ オー チケット GitHub/ディスカッション
ainsi qu'au Formulaire による構造キャプチャ
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)。

|参考資料 |重度の |責任者 |法令 |メモ |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` |低い |ドキュメント-コア-02 |解決 2025-04-18 |ナビゲーションの文言の明確化 + サイドバー (`docs/source/sorafs/tryit.md` は jour avec le nouveau ラベルに誤りがあります)。 |
| `docs-preview/w1 #2` |低い |ドキュメントコア-03 |解決 2025-04-19 | Try it + 伝説のラフラヒエス セロン ラ デマンドをキャプチャします。アーティファクト `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`。 |
| - |情報 |ドキュメント/DevRel リード |フェルム |コメントに関する質問や独自性に関する Q&A; dans Chaque Formulaire partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` をキャプチャします。 |

## スイビの知識チェックとアンケート

1. クイズのスコアを登録する人 (cible >=90%) がレビュー担当者に注ぐ。招待状の CSV に参加して成果物をエクスポートします。
2. 収集者は、フィードバックおよびコピー機のテンプレートを介して調査キャプチャの定性的な応答を行います。
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`。
3. 担当者と荷送人による改善の計画立案者。

Les huit レビュー担当者が 94% 以上の知識をチェック (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`)。 Aucun の救済措置
決して必要なものではありません。フランスの輸出調査
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`。

## 工芸品の発明

- バンドル プレビュー記述子/チェックサム: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- プローブ + リンクチェックを再開: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- プロキシでの変更ログ お試しください: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- テレメトリのエクスポート: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- バンドルテレメトリーの毎日の営業時間: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- フィードバック + アンケートのエクスポート: 査読者による文書の配置
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV 知識のチェックと履歴書: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Garder l'inventaire は、avec l'issue tracker を同期します。工芸品のコピーを共有する
チケットガバナンスは、アクセスシェルを持たない監査人による検証を保証します。