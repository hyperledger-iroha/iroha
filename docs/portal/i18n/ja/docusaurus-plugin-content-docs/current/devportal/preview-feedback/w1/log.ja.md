---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7470bd5478f2e2ebf5e930dc8a9e525ef915e2763023f9ebdd84dfb3ec9c9b1a
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w1-log
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

このログは **W1 パートナープレビュー** の招待 roster、テレメトリー checkpoint、レビュー担当のフィードバックを保持します。
これは [`preview-feedback/w1/plan.md`](./plan.md) の受け入れタスクと、波の tracker エントリ
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md) に紐づきます。招待送信、テレメトリー
スナップショット記録、フィードバック項目のトリアージが行われたら更新し、ガバナンスレビュー担当が外部チケットを追わずに
証跡を再現できるようにします。

## コホート roster

| Partner ID | 申請チケット | NDA受領 | 招待送信 (UTC) | Ack/初回ログイン (UTC) | ステータス | メモ |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ 完了 2025-04-26 | sorafs-op-01; orchestrator docs parity の証跡を重点確認。 |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ 完了 2025-04-26 | sorafs-op-02; Norito/telemetry cross-links を検証。 |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ 完了 2025-04-26 | sorafs-op-03; multi-source failover drills を実施。 |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ 完了 2025-04-26 | torii-int-01; Torii `/v2/pipeline` + Try it cookbook のレビュー。 |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ 完了 2025-04-26 | torii-int-02; Try it スクリーンショット更新に同行 (docs-preview/w1 #2)。 |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ 完了 2025-04-26 | sdk-partner-01; JS/Swift cookbook のフィードバック + ISO bridge sanity checks。 |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ 完了 2025-04-26 | sdk-partner-02; compliance は 2025-04-11 に承認、Connect/telemetry メモに集中。 |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ 完了 2025-04-26 | gateway-ops-01; gateway ops ガイドを監査 + Try it proxy フローを匿名化。 |

**招待送信** と **Ack** の時刻は、外部メール送信後すぐに埋めてください。
W1 計画で定義された UTC スケジュールに合わせます。

## テレメトリー checkpoint

| タイムスタンプ (UTC) | Dashboards / probes | 担当 | 結果 | アーティファクト |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ 全てグリーン | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` の transcript | Ops | ✅ ステージ済み | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | 上記ダッシュボード + `probe:portal` | Docs/DevRel + Ops | ✅ 招待前スナップショット、回帰なし | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | 上記ダッシュボード + Try it proxy レイテンシ差分 | Docs/DevRel lead | ✅ 中間チェック合格 (アラート0; Try it レイテンシ p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | 上記ダッシュボード + exit probe | Docs/DevRel + Governance liaison | ✅ 退出スナップショット、未解決アラート0 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

日次 office hours サンプル (2025-04-13 -> 2025-04-25) は NDJSON + PNG でまとめ、
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` に保存しています。ファイル名は
`docs-preview-integrity-<date>.json` と対応するスクリーンショットです。

## フィードバック/issue ログ

この表でレビュアーの指摘を要約します。各行を GitHub/discuss チケットと、
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)
で取得した構造化フォームにリンクしてください。

| Reference | Severity | Owner | Status | Notes |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Low | Docs-core-02 | ✅ Resolved 2025-04-18 | Try it ナビ文言と sidebar anchor を明確化 (`docs/source/sorafs/tryit.md` を新ラベルに更新)。 |
| `docs-preview/w1 #2` | Low | Docs-core-03 | ✅ Resolved 2025-04-19 | Try it スクリーンショットとキャプションを更新; artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Info | Docs/DevRel lead | 🟢 Closed | 残りのコメントはQ&Aのみ; 各パートナーのフォームに記録済み `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Knowledge check と survey 追跡

1. すべてのレビュアーの quiz スコアを記録 (目標 >=90%); エクスポートした CSV を招待アーティファクトと一緒に添付。
2. フィードバックフォームで取得した定性 survey 回答を収集し、
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` に反映。
3. しきい値未満の人に remediation コールを設定し、ここに記録。

8名全員が knowledge check で >=94% を達成 (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). remediation コールは不要;
各パートナーの survey export は
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json` に保存。

## アーティファクト一覧

- Preview descriptor/checksum bundle: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Probe + link-check summary: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Try it proxy 変更ログ: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Telemetry exports: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Daily office-hour telemetry bundle: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Feedback + survey exports: reviewer 別フォルダを
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` に配置
- Knowledge check CSV と summary: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

インベントリは tracker issue と同期させてください。ガバナンスチケットへコピーする際はハッシュを添付し、
監査担当がシェルアクセスなしで検証できるようにします。
