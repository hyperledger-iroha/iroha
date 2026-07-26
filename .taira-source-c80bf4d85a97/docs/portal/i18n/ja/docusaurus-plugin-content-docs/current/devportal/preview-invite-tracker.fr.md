---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# トラッカーの招待状プレビュー

Ce トラッカーは、オーナーのポートテイル ドキュメントを登録し、DOCS-SORA および les reelecteurs gouvernance voient quelle cohorte est active, qui a approuve les招待状および quels artefacts が裏切り者を保持しています。メッテズ・ル・ア・ジュール・チャック・フォア・ケ・デ・招待状、ソント特使、撤回、報告者は、ピスト・ド・オーディット・レスト・ダン・ル・デポを注ぎます。

## 曖昧な規定

|あいまい |コホート |スイビ号 |承認者 |法令 |フェネトレ・シブル |メモ |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - コアメンテナー** |メンテナ ドキュメント + SDK 検証ファイル フラックス チェックサム | `DOCS-SORA-Preview-W0` (トラッカー GitHub/ops) |リード ドキュメント/DevRel + ポータル TL |ターミナル | 2025 年第 2 四半期セマイネス 1-2 |招待使者は 2025-03-25、テレメトリ休憩者は verte、出撃再開は 2025-04-08 です。 |
| **W1 - パートナー** |オペレーター SoraFS、インテグレーター Torii NDA | `DOCS-SORA-Preview-W1` |リードドキュメント/DevRel + 連絡管理 |ターミナル | 2025 年第 2 四半期セメイン 3 |招待状 2025-04-12 -> 2025-04-26 avec les huit パートナーが確認。 [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) で証拠を捕捉し、[`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) で出撃を再開します。 |
| **W2 - コミュノート** |待機リストの連絡先 (<=25 アラフォワ) | `DOCS-SORA-Preview-W2` |リード ドキュメント/DevRel + コミュニティ マネージャー |ターミナル | 2025 年第 3 四半期 セメイン 1 (テンタティフ) |招待状 2025-06-15 -> 2025-06-29 avec telemetry verte tout du long;証拠 + 統計情報 [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md)。 |
| **W3 - コホート ベータ** |ベータ版のファイナンス/オブザーバビリティ + パートナー SDK + アドボケート エコシステム | `DOCS-SORA-Preview-W3` |リードドキュメント/DevRel + 連絡管理 |ターミナル | 2026 年第 1 四半期 セメイン 8 |招待状 2026-02-18 -> 2026-02-28;履歴書 + donnees ポートテイル ジェネレ (曖昧な `preview-20260218` 経由) ([`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md))。 |

> 注: 問題はトラッカーの補助チケットと要求のプレビューとアーカイブ、プロジェクト `docs-portal-preview` に基づいて承認され、承認されます。

## アクティブ (W0) をタシュします。

- プリフライト ラフライチのアーティファクト (GitHub Actions `docs-portal-preview` 2025-03-24 の実行、`scripts/preview_verify.sh` avec le tag `preview-2025-03-24` による記述子検証)。
- ベースライン テレメトリ キャプチャ (`docs.preview.integrity`、ダッシュボードのスナップショット `TryItProxyErrors` の問題 W0)。
- Texte d'outreach fig avec [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) およびタグ プレビュー `preview-2025-03-24`。
- Demandes d'entre enregistrees pour les cinq premiers メンテナー (チケット `DOCS-SORA-Preview-REQ-01` ... `-05`)。
- Cinq は招待者をプレミア公開します 2025-03-25 10:00-10:20 UTC apres sept jours consecutifs de telemetry verte; accus の在庫は `DOCS-SORA-Preview-W0` です。
- Suvi テレメトリ + ホストのオフィスアワー (チェックイン quotidiens jusqu'au 2025-03-31; log des Checkpoints ci-dessous)。
- 曖昧なフィードバック/収集者とタグ `docs-preview/w0` ([W0 ダイジェスト](./preview-feedback/w0/summary.md)) を収集します。
- 曖昧な公開履歴書 + 出撃確認書 (出撃日 2025-04-08; voir [W0 ダイジェスト](./preview-feedback/w0/summary.md))。
- 漠然としたベータ版の W3 スイビ。先物曖昧な計画、セロンレビューガバナンス。

## 曖昧な W1 パートナーの履歴書

- 法律および統治の承認。追加パートナーは 2025 年 4 月 5 日に署名します。承認料金は `DOCS-SORA-Preview-W1` です。
- テレメトリー + ステージングを試してください。変更チケット `OPS-TRYIT-147` は、2025 年 4 月 6 日の avec スナップショット Grafana、`docs.preview.integrity`、`TryItProxyErrors`、および `DocsPortal/GatewayRefusals` アーカイブを実行します。
- 準備アーティファクト + チェックサム。バンドル `preview-2025-04-12` を確認します。ログ記述子/チェックサム/プローブ ストックは `artifacts/docs_preview/W1/preview-2025-04-12/` です。
- 名簿の招待状 + envoi。 Huit はパートナー (`DOCS-SORA-Preview-REQ-P01...P08`) の承認を要求します。招待特使 2025-04-12 15:00-15:21 UTC avec accus par reelecteur.
- 計測器のフィードバック。オフィスアワーの定例 + チェックポイントのテレメトリが登録されます。 [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) のダイジェストをご覧ください。
- 名簿最終/出撃ログ。 [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) 招待/ACK のメンテナンス タイムスタンプ、証拠のテレメトリー、輸出クイズと成果物を 2025 年 4 月 26 日に許可します。

## 招待状のログ - W0 コア メンテナ| ID再選者 |役割 |チケット・デ・デマンド |招待使者 (UTC) |出撃参加者 (UTC) |法令 |メモ |
| --- | --- | --- | --- | --- | --- | --- |
|ドキュメントコア-01 |ポータル管理者 | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 |アクティフ |検証チェックサムの確認。フォーカス ナビゲーション/サイドバー。 |
| sdk-錆-01 | Rust SDK リード | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 |アクティフ | Teste les rectets SDK + クイックスタート Norito。 |
| sdk-js-01 | JS SDK メンテナー | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 |アクティフ |コンソールで有効にして試してみて、ISO をフローします。 |
| sorafs-ops-01 | SoraFS オペレーター連絡 | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 |アクティフ | Runbook SoraFS + ドキュメント オーケストレーションを監査します。 |
|可観測性-01 |可観測性TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 |アクティフ | Revoit les はテレメトリ/インシデントを添付します。責任あるクーベルチュール アラートマネージャー。 |

招待状参照、ミーム アーティファクト `docs-portal-preview` (実行 2025 年 3 月 24 日、タグ `preview-2025-03-24`) および検証キャプチャ ログ `DOCS-SORA-Preview-W0`。曖昧な問題を解決するために、前もって追跡する必要があるかどうかを確認し、一時停止します。

## チェックポイントのログ - W0

|日付 (UTC) |アクティビティ |メモ |
| --- | --- | --- |
| 2025-03-26 |レビュー テレメトリ ベースライン + 営業時間 | `docs.preview.integrity` + `TryItProxyErrors` ソントレストレスト;オフィスアワーは、検証チェックサムの終了を確認するために必要です。 |
| 2025-03-27 |ダイジェストフィードバック仲介出版 | [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) でキャプチャを再開します。 deux は nav Mineures taguees `docs-preview/w0`、aucun 事件を発行します。 |
| 2025-03-31 |テレメトリーを確認してください。デルニエールのオフィスアワーは出口前に終了します。再選者は、再スタンテスを確認し、警告を確認してください。 |
| 2025-04-08 |出撃の再開 + 招待状の提出 |契約終了確認者、アクセス一時取り消し、統計アーカイブの確認 [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08);トラッカーミスアジュールアバントW1。 |

## 招待状のログ - W1 パートナー

| ID再選者 |役割 |チケット・デ・デマンド |招待使者 (UTC) |出撃参加者 (UTC) |法令 |メモ |
| --- | --- | --- | --- | --- | --- | --- |
|ソラフス-op-01 | SoraFS オペレーター (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 |ターミナル |フィードバック運用オーケストレーター ライブ 2025-04-20; ACK 出撃 15:05 UTC。 |
|ソラフス-op-02 | SoraFS オペレーター (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 |ターミナル | `docs-preview/w1` によるコメントロールアウトログ。 ACK 15:10 UTC。 |
|ソラフス-op-03 | SoraFS オペレーター (米国) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 |ターミナル |異議申し立て/ブラックリストの登録を編集します。 ACK 15:12 UTC。 |
|鳥居-int-01 | Torii インテグレータ | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 |ターミナル |ウォークスルー 試してみて認証を受け入れます。 ACK 15:14 UTC。 |
|鳥居-int-02 | Torii インテグレータ | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 |ターミナル |コメント RPC/OAuth ログ。 ACK 15:16 UTC。 |
| SDK-パートナー-01 | SDK パートナー (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 |ターミナル |フィードバックの完全なプレビューのマージ。 ACK 15:18 UTC。 |
| SDK-パートナー-02 | SDK パートナー (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 |ターミナル |レビューテレメトリ/編集フェイト。 ACK 15:22 UTC。 |
|ゲートウェイ-ops-01 |ゲートウェイオペレーター | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 |ターミナル |コメントランブック DNS ゲートウェイ ログ。 ACK 15:24 UTC。 |

## チェックポイントのログ - W1

|日付 (UTC) |アクティビティ |メモ |
| --- | --- | --- |
| 2025-04-12 | Envoi の招待状 + 検証アーティファクト | Huit パートナーは、avec 記述子/アーカイブ `preview-2025-04-12` を電子メールで送信します。トラッカーによるアカスストック。 |
| 2025-04-13 | Revue テレメトリ ベースライン | `docs.preview.integrity`、`TryItProxyErrors`、および `DocsPortal/GatewayRefusals` 頂点。オフィスアワーは、検証チェックサムの終了を確認するために必要です。 |
| 2025-04-18 |オフィスアワーは曖昧 | `docs.preview.integrity` レスト ヴェール; deux nits docs タグ `docs-preview/w1` (ナビゲーションの文言 + スクリーンショット 試してみてください)。 |
| 2025-04-22 |最終的なテレメトリを確認する |プロキシ + ダッシュボードのサイン。オーキューン ヌーベル号、ル トラッカー アバント出撃に注意してください。 |
| 2025-04-26 |出撃再開＋フェルミチュール招待状 |パートナーは、レビュー、招待状の取り消し、証拠のアーカイブを確認することはできません [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26)。 |

## コホート ベータ W3 の要約- 招待使者 2026-02-18 avec 検証チェックサム + accus le meme jour。
- フィードバックを収集して `docs-preview/20260218` 行政機関 `DOCS-SORA-Preview-20260218` を発行します。 `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` 経由のダイジェスト + レジュームジェネレータ。
- Access revoque 2026 年 2 月 28 日、最終的なテレメトリの確認。トラッカー + テーブル ポータルは、1 時間のマーケール W3 ターミナルを見逃します。

## 招待状のログ - W2 コミュニティ

| ID再選者 |役割 |チケット・デ・デマンド |招待使者 (UTC) |出撃参加者 (UTC) |法令 |メモ |
| --- | --- | --- | --- | --- | --- | --- |
|コム-vol-01 |コミュニティレビュアー (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 |ターミナル | ACK 16:06 UTC;フォーカスクイックスタート SDK;出撃確認者2025-06-29。 |
|コム-vol-02 |コミュニティレビュアー (ガバナンス) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 |ターミナル |レビューガバナンス/SNS終了者。出撃確認者2025-06-29。 |
|コム-vol-03 |コミュニティレビュアー (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 |ターミナル |フィードバック ウォークスルー Norito ログ。 ack 2025-06-29。 |
|コム-vol-04 |コミュニティレビュアー (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 |ターミナル | Revue ランブック SoraFS 終了者。 ack 2025-06-29。 |
|通信vol.05 |コミュニティレビュアー (アクセシビリティ) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 |ターミナル | Notes アクセシビライト/UX の参加者。 ack 2025-06-29。 |
|通信vol.06 |コミュニティレビュアー (ローカリゼーション) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 |ターミナル |フィードバック ローカリゼーション ログ。 ack 2025-06-29。 |
|コム-vol-07 |コミュニティレビュアー (モバイル) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 |ターミナル |ドキュメント SDK モバイル ライブラリをチェックします。 ack 2025-06-29。 |
|通信vol.08 |コミュニティレビュアー (可観測性) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 |ターミナル |レヴュー別館観察可能終点。 ack 2025-06-29。 |

## チェックポイントのログ - W2

|日付 (UTC) |アクティビティ |メモ |
| --- | --- | --- |
| 2025-06-15 | Envoi の招待状 + 検証アーティファクト |記述子/アーカイブ `preview-2025-06-15` 部分平均 8 回の選択。アカスストックとトラッカー。 |
| 2025-06-16 | Revue テレメトリ ベースライン | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` 頂点。ログ プロキシ モントレント トークン コミュニケーション アクティビティを試してみてください。 |
| 2025-06-18 |オフィスアワーと問題のトリアージ | 2 つの提案 (`docs-preview/w2 #1` 文言ツールチップ、`#2` サイドバーのローカリゼーション) - 2 つの担当者にドキュメントを提示します。 |
| 2025-06-21 |テレメトリの確認とドキュメントの修正 | `docs-preview/w2 #1/#2` を説明します。ダッシュボードのバーツ、オーカン事件。 |
| 2025-06-24 |オフィスアワー fin de semaine |再選者はレストランを確認します。アキュネアラート。 |
| 2025-06-29 |出撃再開＋フェルミチュール招待状 |登録確認、取り消しプレビュー、スナップショット + アーティファクト アーカイブへのアクセス ([`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29))。 |
| 2025-04-15 |オフィスアワーと問題のトリアージ | Deux 提案ドキュメント ログ `docs-preview/w1`;オークン事件に注意してください。 |

## レポートのフック

- Chaque mercredi、mettre a jour le tableau ci-dessus et l'issue は、アクティブな avec une note courte を招待します (招待使者、選出者活動、事件)。
- あいまいな終了、履歴書のフィードバック (例: `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) および le lier depuis `status.md` を確認してください。
- [プレビュー招待フロー](./preview-invite-flow.md) を一時停止する基準があり、招待状を修正するための準備が整いました。