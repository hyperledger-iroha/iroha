---
id: preview-feedback-w1-plan
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| 項目 | 詳細 |
| --- | --- |
| 波 | W1 - パートナーとTorii統合担当 |
| 目標ウィンドウ | 2025年Q2 第3週 |
| アーティファクトタグ (予定) | `preview-2025-04-12` |
| トラッカーIssue | `DOCS-SORA-Preview-W1` |

## 目的

1. パートナープレビュー条項の法務・ガバナンス承認を確保する。
2. 招待バンドルで使うTry it proxyとテレメトリースナップショットを準備する。
3. チェックサム検証済みのプレビューアーティファクトとprobe結果を更新する。
4. 招待送付前にパートナーロスターと申請テンプレートを確定する。

## タスク内訳

| ID | タスク | 担当 | 期限 | ステータス | メモ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | プレビュー条項追加の法務承認を取得 | Docs/DevRel lead -> Legal | 2025-04-05 | ✅ 完了 | 法務チケット `DOCS-SORA-Preview-W1-Legal` が2025-04-05に承認; PDFはトラッカーに添付。 |
| W1-P2 | Try it proxyのstagingウィンドウ(2025-04-10)を確保し、プロキシ健全性を確認 | Docs/DevRel + Ops | 2025-04-06 | ✅ 完了 | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` を2025-04-06に実行; CLIトランスクリプトと`.env.tryit-proxy.bak`を保存。 |
| W1-P3 | プレビューアーティファクト(`preview-2025-04-12`)をビルドし、`scripts/preview_verify.sh` + `npm run probe:portal`を実行、descriptor/checksumsを保存 | Portal TL | 2025-04-08 | ✅ 完了 | アーティファクトと検証ログは`artifacts/docs_preview/W1/preview-2025-04-12/`に保存; probe出力をトラッカーに添付。 |
| W1-P4 | パートナーintakeフォーム(`DOCS-SORA-Preview-REQ-P01...P08`)を確認し、連絡先とNDAを確定 | Governance liaison | 2025-04-07 | ✅ 完了 | 8件すべて承認(最後の2件は2025-04-11に承認); 承認はトラッカーにリンク。 |
| W1-P5 | 招待文面を作成(`docs/examples/docs_preview_invite_template.md`ベース)、各パートナーに`<preview_tag>`と`<request_ticket>`を設定 | Docs/DevRel lead | 2025-04-08 | ✅ 完了 | 招待ドラフトを2025-04-12 15:00 UTCに送付; アーティファクトリンクも同送。 |

## Preflightチェックリスト

> ヒント: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` を実行すると、手順1-5を自動実行できます(build、checksum検証、ポータルprobe、link checker、Try it proxy更新)。スクリプトはJSONログを記録し、トラッカーissueに添付できます。

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12`付き)で`build/checksums.sha256`と`build/release.json`を再生成。
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links`を実行し、`build/link-report.json`をdescriptorの隣に保存。
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (必要に応じて`--tryit-target`でtarget指定); 更新した`.env.tryit-proxy`をコミットし、rollback用に`.bak`を保持。
6. W1トラッカーissueをログパスで更新(descriptor checksum、probe出力、Try it proxyの変更、Grafanaスナップショット)。

## 証跡チェックリスト

- [x] 署名済みの法務承認(PDFまたはチケットリンク)を`DOCS-SORA-Preview-W1`に添付。
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`のGrafanaスクリーンショット。
- [x] `preview-2025-04-12`のdescriptorとchecksumログを`artifacts/docs_preview/W1/`に保存。
- [x] `invite_sent_at`を埋めた招待ロスター表(トラッカーW1ログ参照)。
- [x] `preview-feedback/w1/log.md`にフィードバックアーティファクトを反映(パートナーごとに1行; 2025-04-26にroster/telemetria/issuesデータで更新)。

この計画は進行に合わせて更新してください。トラッカーは監査可能なroadmap維持のために参照します。

## フィードバック手順

1. レビュアーごとに
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)
   のテンプレートを複製し、メタデータを記入して
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`に保存。
2. 招待、テレメトリーチェックポイント、未解決issueを
   [`preview-feedback/w1/log.md`](./log.md)のライブログにまとめ、ガバナンスレビュー担当が
   リポジトリ内で波全体を再現できるようにする。
3. knowledge-checkやサーベイのエクスポートが届いたら、ログに記載されたアーティファクトパスへ添付し、
   トラッカーissueにリンクする。
