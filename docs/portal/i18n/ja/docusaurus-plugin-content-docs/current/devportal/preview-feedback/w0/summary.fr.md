---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w0-まとめ
タイトル: Digest des retours mi-parcours W0
サイドバーラベル: ルトゥール W0 (ミパルクール)
説明: 管理者コアの曖昧なプレビューを提供する、管理ポイント、統計情報、およびアクションの管理。
---

|要素 |詳細 |
| --- | --- |
|あいまい | W0 - メンテナコア |
|ダイジェストの日付 | 2025-03-27 |
|フェネトレ・ド・レビュー | 2025-03-25 -> 2025-04-08 |
|参加者 | docs-core-01、sdk-rust-01、sdk-js-01、sorafs-ops-01、observability-01 |
|タグ・ダルティファクト | `preview-2025-03-24` |

## ポイントの船員

1. **チェックサムのワークフロー** - レビュー担当者が確認しない `scripts/preview_verify.sh`
   reussi contre le couple 記述子/アーカイブ部分。 Aucun はマヌエルの要求をオーバーライドします。
2. **ナビゲーションの詳細** - シグナルに関するサイドバーの詳細な問題
   (`docs-preview/w0 #1-#2`)。ルートと Docs/DevRel およびブロック パスの詳細
   曖昧な。
3. **ランブック SoraFS** のパリテ - sorafs-ops-01 クロワーズとクレアの要求
   `sorafs/orchestrator-ops` と `sorafs/multi-source-rollout` を入力してください。問題を解決します。
   トレイターアバントW1。
4. **遠隔測定のレビュー** - 可観測性-01 確認クエリ `docs.preview.integrity`、
   `TryItProxyErrors` プロキシのログを試してみてください。オーキュン・アラート・ナ
   エテ・デクレンシェ。

## アクション

| ID |説明 |責任者 |法令 |
| --- | --- | --- | --- |
| W0-A1 | Reordonner les entrees du Sidebar du devportal pour metre en avant les docs pour reviewers (`preview-invite-*` regroupes)。 |ドキュメントコア-01 |終了 - サイドバーのリスト、メンテナンス、ドキュメントのレビュー担当者の継続的な情報 (`docs/portal/sidebars.js`)。 |
| W0-A2 | Ajouter un lien croise、`sorafs/orchestrator-ops` et `sorafs/multi-source-rollout` を明示的に指定してください。 |ソラフス-ops-01 |テルミネ - チャック ランブック ポワント デソルメ 対 ロートル フォア ケ レ オペレーター ボイエン レ ドゥ ガイド ペンダント レ ロールアウト。 |
| W0-A3 |テレメトリのスナップショットとガバナンスのトラッカーのリクエストをバンドルします。 |可観測性-01 |終了 - `DOCS-SORA-Preview-W0` をバンドル添付します。 |

## 出撃再開 (2025-04-08)

- レビュー担当者はラ・フィンを確認せず、ロコーの建物を削除し、フェネトル・ドをやめてください
  プレビュー; `DOCS-SORA-Preview-W0` による登録取り消しの申請。
- Aucun事件に警告ペンダントla曖昧;テレメトリのダッシュボードの残りの部分
  ペンダント トゥート ラ ピリオド。
- ナビゲーションのアクションとクロワーズの先取特権 (W0-A1/A2) 実施者およびリフレ員のソント
  les docs ci-dessus; la preuve telemetry (W0-A3) EST 添付者 au トラッカー。
- プルーブ アーカイブのバンドル: テレメトリーのスクリーンショット、招待状の告発などのダイジェスト
  追跡者の問題はありません。

## プロカインエテープ

- 実装者はアクション W0 アバンドゥブリル W1 を実行します。
- ステージングの代理人として合法的かつスロットの承認を取得し、管理者を保護します
  曖昧なパートナーの詳細を説明するプレフライト [プレビュー招待フロー](../../preview-invite-flow.md)。

_Ce ダイジェスト est lie depuis le [プレビュー招待トラッカー](../../preview-invite-tracker.md) を注ぐ
ガーダー ル ロードマップ DOCS-SORA 追跡可能。_