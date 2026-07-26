---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/preview-invite-tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 77aced74849f58abc05edcfa2005215c1f045c47258d3bd2d124536ae53dae79
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# プレビュー招待トラッカー

このトラッカーは、DOCS-SORA のオーナーとガバナンスレビュアーが、どのコホートがアクティブで、誰が招待を承認し、どのアーティファクトが未対応かを確認できるように、ポータルのプレビュー波を記録します。招待の送信、取り消し、延期が発生するたびに更新し、監査の履歴をリポジトリ内に保ちます。

## ウェーブ状況

| ウェーブ | コホート | トラッカー issue | 承認者 | ステータス | 目標ウィンドウ | メモ |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - コアメンテナー** | checksum フローを検証する Docs + SDK メンテナー | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Docs/DevRel lead + Portal TL | 完了 | Q2 2025 週1-2 | 招待は 2025-03-25 に送信、テレメトリーは緑のまま、退出サマリは 2025-04-08 に公開。 |
| **W1 - パートナー** | NDA 下の SoraFS オペレーター、Torii インテグレーター | `DOCS-SORA-Preview-W1` | Docs/DevRel lead + Governance liaison | 完了 | Q2 2025 週3 | 招待は 2025-04-12 -> 2025-04-26、8 社すべて承認済み; 証拠は [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md)、退出ダイジェストは [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md)。 |
| **W2 - コミュニティ** | キュレーションされたコミュニティ待機リスト (<=25 同時) | `DOCS-SORA-Preview-W2` | Docs/DevRel lead + Community manager | 完了 | Q3 2025 週1 (暫定) | 招待は 2025-06-15 -> 2025-06-29、期間中テレメトリーは緑; 証拠と所見は [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md)。 |
| **W3 - ベータコホート** | 財務/可観測性ベータ + SDK パートナー + エコシステム推進者 | `DOCS-SORA-Preview-W3` | Docs/DevRel lead + Governance liaison | 完了 | Q1 2026 週8 | 招待は 2026-02-18 -> 2026-02-28; `preview-20260218` ウェーブで生成したダイジェスト + ポータルデータ (参照: [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md))。 |

> 注記: 各トラッカー issue を対応するプレビューリクエストチケットにリンクし、`docs-portal-preview` プロジェクトにアーカイブして承認履歴を追跡可能にしてください。

## アクティブタスク (W0)

- preflight 成果物を更新 (GitHub Actions `docs-portal-preview` 2025-03-24 実行、`scripts/preview_verify.sh` で descriptor を `preview-2025-03-24` タグとして検証)。
- テレメトリー baseline を取得 (`docs.preview.integrity`, `TryItProxyErrors` のダッシュボード snapshot を W0 issue に保存)。
- outreach 文面を [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) で固定し、preview タグ `preview-2025-03-24` を設定。
- 最初の 5 名のメンテナーのリクエストを記録 (チケット `DOCS-SORA-Preview-REQ-01` ... `-05`).
- 最初の 5 通の招待を 2025-03-25 10:00-10:20 UTC に送信。7 日連続でテレメトリーが緑、確認は `DOCS-SORA-Preview-W0` に保存。
- テレメトリー監視 + host の office hours を実施 (2025-03-31 まで日次チェック; 下に checkpoints ログ)。
- 中間フィードバック / issues を収集し `docs-preview/w0` を付与 (参照: [W0 digest](./preview-feedback/w0/summary.md)).
- ウェーブのサマリと招待終了確認を公開 (退出バンドル 2025-04-08; 参照: [W0 digest](./preview-feedback/w0/summary.md)).
- W3 ベータウェーブを追跡; 以降のウェーブはガバナンスレビュー後にスケジュール。

## W1 パートナー波の要約

- 法務とガバナンス承認。パートナー addendum は 2025-04-05 に署名され、承認書は `DOCS-SORA-Preview-W1` にアップロード。
- テレメトリー + Try it staging。変更チケット `OPS-TRYIT-147` を 2025-04-06 に実行し、`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` の Grafana snapshot をアーカイブ。
- artefact + checksum 準備。`preview-2025-04-12` バンドルを検証し、descriptor/checksum/probe ログを `artifacts/docs_preview/W1/preview-2025-04-12/` に保存。
- 招待ロスター + 送信。8 件のパートナー申請 (`DOCS-SORA-Preview-REQ-P01...P08`) を承認し、2025-04-12 15:00-15:21 UTC に送信、各レビュアーの ack を記録。
- フィードバック計測。日次 office hours + テレメトリー checkpoints を記録; ダイジェストは [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md)。
- 最終ロスター / 退出ログ。[`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) に招待/ack の時刻、テレメトリー証拠、quiz の exports、artefact 参照が 2025-04-26 時点で記録され、ガバナンスが再現可能。

## 招待ログ - W0 core maintainers

| レビュアー ID | 役割 | 申請チケット | 招待送信 (UTC) | 退出予定 (UTC) | ステータス | メモ |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Active | checksum 検証を確認; nav/sidebar レビューに集中。 |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Active | SDK レシピと Norito quickstarts を検証。 |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Active | Try it コンソール + ISO フローを確認。 |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Active | SoraFS runbooks + orchestration docs を監査。 |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Active | テレメトリー/インシデント付録をレビュー; Alertmanager カバレッジ担当。 |

すべての招待は同じ `docs-portal-preview` artefact (実行 2025-03-24、タグ `preview-2025-03-24`) と `DOCS-SORA-Preview-W0` に保存された検証トランスクリプトを参照します。追加/一時停止は、次のウェーブに進む前に上の表とトラッカー issue の両方へ記録してください。

## チェックポイントログ - W0

| 日付 (UTC) | 活動 | メモ |
| --- | --- | --- |
| 2025-03-26 | テレメトリーベースラインレビュー + office hours | `docs.preview.integrity` と `TryItProxyErrors` は緑を維持; office hours で checksum 検証完了を確認。 |
| 2025-03-27 | 中間フィードバック digest 公開 | [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) にサマリを保存; nav の小さな問題 2 件を `docs-preview/w0` として記録、インシデントなし。 |
| 2025-03-31 | 最終週のテレメトリーチェック | 退出前の最終 office hours; レビュアーは残タスク進行中、アラートなし。 |
| 2025-04-08 | 退出サマリ + 招待クローズ | レビュー完了を確認し、一時アクセスを取り消し、結果を [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) に保存; W1 準備前にトラッカー更新。 |

## 招待ログ - W1 partners

| レビュアー ID | 役割 | 申請チケット | 招待送信 (UTC) | 退出予定 (UTC) | ステータス | メモ |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Completed | orchestrator ops のフィードバック提出 2025-04-20; exit ack 15:05 UTC. |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Completed | rollout コメントを `docs-preview/w1` に記録; ack 15:10 UTC. |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Completed | dispute/blacklist 編集を記録; ack 15:12 UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Completed | Try it auth walkthrough を承認; ack 15:14 UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Completed | RPC/OAuth コメントを記録; ack 15:16 UTC. |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Completed | preview integrity のフィードバックをマージ; ack 15:18 UTC. |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Completed | telemetria/redaction のレビュー完了; ack 15:22 UTC. |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Completed | gateway DNS runbook コメントを記録; ack 15:24 UTC. |

## チェックポイントログ - W1

| 日付 (UTC) | 活動 | メモ |
| --- | --- | --- |
| 2025-04-12 | 招待送信 + artefact 検証 | 8 社に `preview-2025-04-12` descriptor/archive を送信; ack はトラッカーに保存。 |
| 2025-04-13 | テレメトリーベースラインレビュー | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` は緑; office hours で checksum 検証完了を確認。 |
| 2025-04-18 | 中間 office hours | `docs.preview.integrity` は緑を維持; docs の小さな指摘 2 件を `docs-preview/w1` として記録 (nav wording + Try it screenshot)。 |
| 2025-04-22 | 最終テレメトリーチェック | プロキシとダッシュボードは健全; 新規 issue なし、退出前にトラッカーへ記録。 |
| 2025-04-26 | 退出サマリ + 招待クローズ | 全パートナーがレビュー完了を確認、招待を取り消し、証拠を [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) に保存。 |

## W3 ベータコホートのまとめ

- 2026-02-18 に招待送信、checksum 検証と ack を同日記録。
- `docs-preview/20260218` でフィードバック収集、ガバナンス issue `DOCS-SORA-Preview-20260218` を使用; `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` で digest + summary を生成。
- 2026-02-28 にアクセス撤回。最終テレメトリーチェック後、トラッカーとポータル表を更新して W3 完了を反映。

## 招待ログ - W2 community

| レビュアー ID | 役割 | 申請チケット | 招待送信 (UTC) | 退出予定 (UTC) | ステータス | メモ |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Completed | Ack 16:06 UTC; SDK quickstarts を確認; 2025-06-29 に退出確認。 |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Completed | governance/SNS レビュー完了; 2025-06-29 に退出確認。 |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Completed | Norito walkthrough のフィードバック記録; ack 2025-06-29. |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Completed | SoraFS runbook レビュー完了; ack 2025-06-29. |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Completed | Accessibility/UX ノート共有; ack 2025-06-29. |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Completed | Localization フィードバック記録; ack 2025-06-29. |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Completed | Mobile SDK docs チェック完了; ack 2025-06-29. |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Completed | Observability 付録レビュー完了; ack 2025-06-29. |

## チェックポイントログ - W2

| 日付 (UTC) | 活動 | メモ |
| --- | --- | --- |
| 2025-06-15 | 招待送信 + artefact 検証 | `preview-2025-06-15` descriptor/archive を 8 名に共有; ack はトラッカーに保存。 |
| 2025-06-16 | テレメトリーベースラインレビュー | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` は緑; Try it プロキシログでコミュニティトークンがアクティブ。 |
| 2025-06-18 | office hours & issue トリアージ | 提案 2 件 (`docs-preview/w2 #1` tooltip wording, `#2` localization sidebar) - いずれも Docs に割り当て。 |
| 2025-06-21 | テレメトリーチェック + docs 修正 | Docs が `docs-preview/w2 #1/#2` を対応; ダッシュボードは緑、インシデントなし。 |
| 2025-06-24 | 最終週 office hours | レビュアーが残りのフィードバック提出を確認; アラートなし。 |
| 2025-06-29 | 退出サマリ + 招待クローズ | ack 記録、preview アクセス撤回、snapshots + artefacts をアーカイブ (参照: [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | office hours & issue トリアージ | 文書提案 2 件を `docs-preview/w1` に記録; インシデントやアラートなし。 |

## レポート hooks

- 毎週水曜に、上の表とアクティブ invite issue を短いステータスノート (送信済み招待数、アクティブレビュアー、インシデント) で更新。
- ウェーブ終了時にフィードバックサマリのパス (例: `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) を追加し、`status.md` からリンク。
- [preview invite flow](./preview-invite-flow.md) の一時停止条件が発動した場合、再開前に remediation 手順をここに記録。
