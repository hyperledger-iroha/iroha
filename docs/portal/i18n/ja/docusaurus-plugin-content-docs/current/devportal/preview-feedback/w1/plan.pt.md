---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w1-plan
タイトル: プラノ・デ・プリフライト・デ・パルセイロス W1
サイドバーラベル: プラノ W1
説明: Tarefas は、パルセイロスのプレビューに関する証拠のチェックリストと応答を保存します。
---

|アイテム |デタルヘス |
| --- | --- |
|恩田 | W1 - パルセイロス エ インテグラドーレス Torii |
|ジャネラ・アルボ | 2025 年第 2 四半期セマナ 3 |
| Tag de artefato (プラネハド) | `preview-2025-04-12` |
| do トラッカーの問題 | `DOCS-SORA-Preview-W1` |

## オブジェクト

1. パルセイロスのプレビューに関する政府の法的承認を保証する。
2. プロキシを準備します。バンドルなしでテレメトリのスナップショットを試してみます。
3. プローブのチェックサムのプレビュー検証の最新情報。
4. 招待状を提出するために、パルセイロスの名簿や OS テンプレートを最終決定します。

## デスドブラメント デ タレファス

| ID |タレファ |返信 |プラゾ |ステータス |メモ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 |プレビューを確認し、法的条件を確認してください。ドキュメント/DevRel リード -> 法務 | 2025-04-05 |結論 |チケット法的 `DOCS-SORA-Preview-W1-Legal` は 2025 年 4 月 5 日に承認されました。 PDF anexado ao トラッカー。 |
| W1-P2 |プロキシのステージングをキャプチャーする 試してみてください (2025-04-10) プロキシを検証する |ドキュメント/DevRel + オペレーション | 2025-04-06 |結論 | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 2025 年 4 月 6 日実行。 CLI e `.env.tryit-proxy.bak` アルキバドスの転写。 |
| W1-P3 |プレビュー (`preview-2025-04-12`)、ロッド `scripts/preview_verify.sh` + `npm run probe:portal`、arquivar 記述子/チェックサムの作成 |ポータルTL | 2025-04-08 |結論 | `artifacts/docs_preview/W1/preview-2025-04-12/` のアルマゼナド検証に関するアルテファトのログ。サイダ・デ・プローブ・アネクサダ・アオ・トラッカー。 |
| W1-P4 |パルセイロス摂取規則の改訂 (`DOCS-SORA-Preview-REQ-P01...P08`)、NDA の確認 |ガバナンス連絡窓口 | 2025-04-07 |結論 | as oito solicitacoes aprovadas (as duas ultimas em 2025-04-11);トラッカーなしのリンクです。 |
| W1-P5 | Redigir o convite (baseado em `docs/examples/docs_preview_invite_template.md`)、definir `<preview_tag>` e `<request_ticket>` para cada parceiro |ドキュメント/DevRel リード | 2025-04-08 |結論 | Rascunho do convite enviado em 2025-04-12 15:00 UTC junto com links de artefato. |

## プリフライトのチェックリスト

> Dica: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` パラエグゼキュータの自動実行 OS パス 1 ～ 5 に乗りました (ビルド、チェックサムの検証、ポータルのプローブ、プロキシのリンク チェッカーの自動実行を試してみてください)。 O スクリプト レジストラ UM ログ JSON QUE VOCE Pode anexar ao issue do tracker。

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) パラ再生 `build/checksums.sha256` および `build/release.json`。
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e arquivar `build/link-report.json` ao lado do 記述子。
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (`--tryit-target` 経由でターゲットを渡す); `.bak` パラロールバックを実行する `.env.tryit-proxy` をコミットします。
6. W1 com Caminhos de logs を発行します (チェックサム ドゥ ディスクリプタ、サイド プローブ、プロキシなし Try it e スナップショット Grafana)。

## 証拠のチェックリスト

- [x] Aprovacao 法的アシナダ (PDF リンク チケット) anexada ao `DOCS-SORA-Preview-W1`。
- [x] `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` の Grafana のスクリーンショット。
- [x] `preview-2025-04-12` のチェックサム ログと `artifacts/docs_preview/W1/` の記述子。
- [x] 招待者名簿の表 `invite_sent_at` 事前登録 (バージョン ログ W1 トラッカーなし)。
- [x] フィードバック refletidos em [`preview-feedback/w1/log.md`](./log.md) com uma linha por parceiro (atualizado em 2025-04-26 com mados de roster/telemetria/issues)。

エステ・プラノ・コンフォーメをタレファス・アヴァンカレムとして実現します。 o トラッカー o 参照パラマンター o ロードマップ監査。

## Fluxo のフィードバック

1. パラ CAD レビュアー、テンプレートの複製
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)、
   メタダドスとアルマゼナのコピーを完全なものにする
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。
2. レスミール氏が招集、テレメトリーのチェックポイント、異常な生存記録を発行
   [`preview-feedback/w1/log.md`](./log.md) 政府の査読者のパラグアイ・ポッサム・レヴァー・トゥダ・オンダ
   sem sair do リポジトリ。
3. Quando os は、知識チェックや調査を輸出し、ログのない情報を提供します。
   リンカーまたは発行トラッカー。