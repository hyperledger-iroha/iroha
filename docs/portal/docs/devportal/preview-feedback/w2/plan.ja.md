---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9a599a71cc49432334dbf323125756fc6056414a4b8f7622d4cc69edcfbd7503
source_last_modified: "2025-11-11T05:15:49.840830+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w2-plan
title: W2 コミュニティ intake 計画
sidebar_label: W2 計画
description: コミュニティプレビューコホート向けの intake、承認、証跡チェックリスト。
---

| 項目 | 詳細 |
| --- | --- |
| 波 | W2 - コミュニティレビュアー |
| 目標ウィンドウ | 2025年Q3 第1週 (暫定) |
| アーティファクトタグ (予定) | `preview-2025-06-15` |
| トラッカーIssue | `DOCS-SORA-Preview-W2` |

## 目的

1. コミュニティ intake 基準と vetting フローを定義する。
2. 提案 roster と acceptable-use addendum に対する governance 承認を得る。
3. checksum 検証済み preview artefact とテレメトリーバンドルを新しいウィンドウ向けに更新する。
4. 招待送信前に Try it proxy とダッシュボードを用意する。

## タスク内訳

| ID | タスク | 担当 | 期限 | ステータス | メモ |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | コミュニティ intake 基準(適格性, max slots, CoC要件)を作成し governance に共有 | Docs/DevRel lead | 2025-05-15 | ✅ 完了 | intake ポリシーは `DOCS-SORA-Preview-W2` にマージされ、2025-05-20 の評議会で承認済み。 |
| W2-P2 | コミュニティ向けの質問(動機、稼働、ローカライズ要件)を含めた申請テンプレートに更新 | Docs-core-01 | 2025-05-18 | ✅ 完了 | `docs/examples/docs_preview_request_template.md` に Community セクションが追加され、intake フォームから参照。 |
| W2-P3 | intake 計画の governance 承認を確保 (会議投票 + 記録議事録) | Governance liaison | 2025-05-22 | ✅ 完了 | 2025-05-20 に全会一致で可決; 議事録と roll call は `DOCS-SORA-Preview-W2` にリンク。 |
| W2-P4 | W2 ウィンドウ向けに Try it proxy の staging とテレメトリー取得を予定 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | ✅ 完了 | 変更チケット `OPS-TRYIT-188` は 2025-06-09 02:00-04:00 UTC に承認/実行; Grafana スクリーンショットはチケットに保存。 |
| W2-P5 | 新しい preview artefact tag (`preview-2025-06-15`) をビルド/検証し、descriptor/checksum/probe logs をアーカイブ | Portal TL | 2025-06-07 | ✅ 完了 | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` を 2025-06-10 に実行; 出力は `artifacts/docs_preview/W2/preview-2025-06-15/` に保存。 |
| W2-P6 | コミュニティ招待 roster を編成 (<=25 reviewers, 段階的バッチ) し、governance 承認済み連絡先を含める | Community manager | 2025-06-10 | ✅ 完了 | 最初の 8 名が承認され、申請 ID `DOCS-SORA-Preview-REQ-C01...C08` がトラッカーに記録済み。 |

## 証跡チェックリスト

- [x] governance 承認記録 (会議メモ + 投票リンク) を `DOCS-SORA-Preview-W2` に添付。
- [x] 更新済み request template を `docs/examples/` にコミット。
- [x] `preview-2025-06-15` の descriptor, checksum log, probe output, link report, Try it proxy transcript を `artifacts/docs_preview/W2/` に保管。
- [x] W2 preflight ウィンドウ向けの Grafana スクリーンショット (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) を取得。
- [x] 招待 roster 表に reviewer IDs, request tickets, approval timestamps を記入 (送信前; tracker の W2 セクション参照)。

この計画は常に更新してください。トラッカーが参照し、W2 招待前に何が残っているかを DOCS-SORA ロードマップに明示します。
