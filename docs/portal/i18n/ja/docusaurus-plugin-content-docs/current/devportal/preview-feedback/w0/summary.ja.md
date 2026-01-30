---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w0/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ad170fd9ddeded84dd6ed40faece344ffd36fb86114d3eb490ccb816985c2c07
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w0-summary
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| 項目 | 詳細 |
| --- | --- |
| 波 | W0 - コアメンテナ |
| ダイジェスト日付 | 2025-03-27 |
| レビュー期間 | 2025-03-25 -> 2025-04-08 |
| 参加者 | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| アーティファクトタグ | `preview-2025-03-24` |

## ハイライト

1. **Checksum ワークフロー** - 全てのレビュアーが `scripts/preview_verify.sh` が
   共通の descriptor/archive ペアで成功したことを確認。手動の override は不要。
2. **ナビゲーションのフィードバック** - サイドバー順序の軽微な問題が 2 件報告
   された (`docs-preview/w0 #1-#2`)。どちらも Docs/DevRel に割り当て済みで、
   波をブロックしない。
3. **SoraFS runbook の整合** - sorafs-ops-01 は `sorafs/orchestrator-ops` と
   `sorafs/multi-source-rollout` のクロスリンクをより明確にするよう要望。
   追跡 issue を作成済みで、W1 前に対応予定。
4. **テレメトリーレビュー** - observability-01 が `docs.preview.integrity`,
   `TryItProxyErrors`, Try-it proxy のログがグリーンのままだったことを確認。
   アラートは発火せず。

## 対応事項

| ID | 説明 | 担当 | ステータス |
| --- | --- | --- | --- |
| W0-A1 | devportal の sidebar 項目を並べ替え、レビュー向けドキュメントを目立たせる (`preview-invite-*` をまとめる)。 | Docs-core-01 | 完了 - sidebar がレビュードキュメントを連続して表示するようになった (`docs/portal/sidebars.js`). |
| W0-A2 | `sorafs/orchestrator-ops` と `sorafs/multi-source-rollout` の明示的なクロスリンクを追加する。 | Sorafs-ops-01 | 完了 - 各 runbook が相互にリンクし、rollout 中に両方のガイドを確認できる。 |
| W0-A3 | テレメトリースナップショット + クエリーバンドルを governance tracker と共有する。 | Observability-01 | 完了 - バンドルを `DOCS-SORA-Preview-W0` に添付。 |

## 終了サマリー (2025-04-08)

- 5 名全員が完了を確認し、ローカルビルドを削除して preview ウィンドウを終了。
  アクセス取り消しは `DOCS-SORA-Preview-W0` に記録済み。
- 波の期間中にインシデントやアラートはなく、テレメトリーダッシュボードは
  全期間グリーンを維持。
- ナビゲーション + クロスリンクの対応 (W0-A1/A2) は実装済みで上記 docs に反映。
  テレメトリーの証跡 (W0-A3) は tracker に添付済み。
- 証跡バンドルを保管済み: テレメトリーのスクリーンショット、招待の確認、そして
  このダイジェストは tracker issue からリンクされている。

## 次のステップ

- W0 の対応事項を W1 開始前に実装する。
- 法務承認と proxy の staging 枠を確保し、その後 [preview invite flow](../../preview-invite-flow.md)
  に記載された partner wave の preflight 手順に従う。

_このダイジェストは [preview invite tracker](../../preview-invite-tracker.md) からリンクされ、
DOCS-SORA の roadmap を追跡可能にするためのものです。_
