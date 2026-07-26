---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w1-plan
タイトル: 飛行前パートナー計画 W1
サイドバーラベル: プラン W1
説明: タスク、責任者、およびプレビュー パートナーの協力のためのチェックリスト。
---

|要素 |詳細 |
| --- | --- |
|あいまい | W1 - パートナーと統合者 Torii |
|フェネトレ・シブル | 2025 年第 2 四半期セメイン 3 |
|タグダルティファクト (planifie) | `preview-2025-04-12` |
|問題追跡ツール | `DOCS-SORA-Preview-W1` |

## 目的

1. 法的承認およびガバナンスに関するプレビュー契約を取得します。
2. プロキシの作成者は、招待状のバンドルを利用してテレメトリのスナップショットやスナップショットを試してみます。
3. チェックサムとプローブの結果をプレビュー検証するためのアーティファクトを作成します。
4. 参加者名簿および招待状の事前要求のテンプレートをファイナライズします。

## デコパージュ デ タッシュ

| ID |タチェ |責任者 |エシャンス |法令 |メモ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 |プレビューの追加条項を適用する法的承認を取得する |ドキュメント/DevRel リード -> 法務 | 2025-04-05 |ターミナル |チケット法定 `DOCS-SORA-Preview-W1-Legal` 有効期間 2025 年 4 月 5 日。 auトラッカーをPDF添付。 |
| W1-P2 |プロキシのステージングをキャプチャーする 試してみてください (2025-04-10) およびプロキシの検証者 |ドキュメント/DevRel + オペレーション | 2025-04-06 |ターミナル | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` ファイル 2025-04-06 を実行します。転写 CLI + `.env.tryit-proxy.bak` アーカイブ。 |
| W1-P3 |プレビューのアーティファクト (`preview-2025-04-12`)、実行者 `scripts/preview_verify.sh` + `npm run probe:portal`、アーカイバ記述子/チェックサムを構築します。ポータルTL | 2025-04-08 |ターミナル |アーティファクト + 検証ログ `artifacts/docs_preview/W1/preview-2025-04-12/` をストックします。探査武官オートラッカー出撃。 |
| W1-P4 | Revoir les Formulaires d'intake partenaires (`DOCS-SORA-Preview-REQ-P01...P08`)、確認者連絡先 + NDA |ガバナンス連絡窓口 | 2025-04-07 |ターミナル | Les huit requestes approuvees (les deux dernieres le 2025-04-11)。評価は追跡者に嘘をつきます。 |
| W1-P5 |招待状のテキスト (`docs/examples/docs_preview_invite_template.md` のベース)、`<preview_tag>` と `<request_ticket>` の招待状の定義 |ドキュメント/DevRel リード | 2025-04-08 |ターミナル | Brouillon d'invitation envoye le 2025-04-12 15:00 UTC avec les liens d'artefact。 |

## チェックリストのプリフライト

> Astuce: lancez `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` 実行プログラム自動ファイル 1 ～ 5 (ビルド、検証チェックサム、ポータルのプローブ、リンク チェッカー、プロキシの実行など)。このスクリプトは、JSON のログを登録し、トラッカーに結合します。

1. `npm run build` (avec `DOCS_RELEASE_TAG=preview-2025-04-12`) 再生器 `build/checksums.sha256` および `build/release.json` を注ぎます。
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` およびアーカイバ `build/link-report.json` は記述子です。
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (`--tryit-target` 経由で適切な 4 つを選択);コミッター le `.env.tryit-proxy` は、ロールバックを行う `.bak` のジャーナルとコンサーバーをミスしました。
6. 1 時間ごとに W1 の安全なログを発行します (記述子のチェックサム、プローブの出撃、プロキシの変更、Try it とスナップショット Grafana)。

## 事前チェックリスト

- [x] 承認法的署名者 (PDF ou lien du ticket) 添付者 a `DOCS-SORA-Preview-W1`。
- [x] スクリーンショット Grafana、`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`。
- [x] 記述子とチェックサムのログ `preview-2025-04-12` は、`artifacts/docs_preview/W1/` をストックします。
- [x] 招待状のテーブル リスト、平均 `invite_sent_at` レンセーニュ (ログ W1 デュ トラッカーを参照)。
- [x] フィードバック再現のアーティファクト [`preview-feedback/w1/log.md`](./log.md) avec une ligne par partenaire (mis a jour 2025-04-26 avec roster/telemetria/issues)。

時間をかけて計画を立て、事前に計画を立てます。トラッカーは監査可能なロードマップを参照します。

## フィードバックの流動性

1. チャック レビュアー、デュプリケ ル テンプレートを注ぐ
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)、
   レムプリル レ メタドンネとストッカー ラ コピー コンプリート スー
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。
2. 履歴書の招待状、テレメトリーのチェックポイント、およびログ ヴィヴァンの発行に関する問題
   [`preview-feedback/w1/log.md`](./log.md) 審査員のガバナンスを注ぐ、曖昧なレジュエを提供する
   ル・デポを辞める必要はありません。
3. 知識の輸出や到着の確認、ログの作成や化学製品の記録の確認
   トラッカーの問題について。