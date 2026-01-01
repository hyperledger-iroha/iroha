---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ed6531e0088cf68b6e50d5b3356d1ef96ed6986ff9b46629a2632fa0cc81da65
source_last_modified: "2025-11-10T19:37:41.905393+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w1-summary
title: W1 パートナーフィードバックと終了サマリー
sidebar_label: W1 サマリー
description: パートナー/Torii統合担当のプレビュー波に関する所見、対応、終了証跡。
---

| 項目 | 詳細 |
| --- | --- |
| 波 | W1 - パートナーとTorii統合担当 |
| 招待期間 | 2025-04-12 -> 2025-04-26 |
| アーティファクトタグ | `preview-2025-04-12` |
| トラッカーIssue | `DOCS-SORA-Preview-W1` |
| 参加者 | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## ハイライト

1. **Checksum ワークフロー** - 全てのレビュアーが `scripts/preview_verify.sh` で descriptor/archive を検証; ログは招待承認と併せて保存。
2. **テレメトリー** - `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` のダッシュボードは全期間グリーン; インシデントやアラートページはなし。
3. **ドキュメントフィードバック (`docs-preview/w1`)** - 2件の軽微な指摘:
   - `docs-preview/w1 #1`: Try it セクションのナビ文言を明確化 (解決済み)。
   - `docs-preview/w1 #2`: Try it スクリーンショット更新 (解決済み)。
4. **Runbook 整合** - SoraFS オペレーターは `orchestrator-ops` と `multi-source-rollout` の新しいクロスリンクが W0 の懸念を解消したと確認。

## アクションアイテム

| ID | 内容 | 担当 | ステータス |
| --- | --- | --- | --- |
| W1-A1 | `docs-preview/w1 #1` に合わせて Try it ナビ文言を更新。 | Docs-core-02 | ✅ 完了 (2025-04-18)。 |
| W1-A2 | `docs-preview/w1 #2` に合わせて Try it スクリーンショットを更新。 | Docs-core-03 | ✅ 完了 (2025-04-19)。 |
| W1-A3 | パートナー所見とテレメトリー証跡を roadmap/status に反映。 | Docs/DevRel lead | ✅ 完了 (tracker + status.md 参照)。 |

## 終了サマリー (2025-04-26)

- 8名全員が最終オフィスアワーで完了を確認し、ローカルアーティファクトを削除し、アクセスは取り消された。
- テレメトリーは終了までグリーンを維持; 最終スナップショットは `DOCS-SORA-Preview-W1` に添付。
- 招待ログを退出確認で更新; トラッカーはW1を🈴に更新し、チェックポイントを追加。
- 証跡バンドル (descriptor, checksum log, probe output, Try it proxy transcript, telemetry screenshots, feedback digest) を `artifacts/docs_preview/W1/` に保存。

## 次のステップ

- W2コミュニティ intake 計画を準備 (governance承認 + request template調整)。
- W2波向けに preview artefact tag を更新し、日程確定後に preflight スクリプトを再実行。
- W1の適用可能な所見を roadmap/status に反映し、コミュニティ波へ最新ガイダンスを提供。
