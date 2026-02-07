---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w1-plan
タイトル: パートナー W1 のプリフライト計画
サイドバーラベル: プラン W1
説明: 担当者およびパートナーのプレビューに関する証拠のチェックリスト。
---

|アイテム |詳細 |
| --- | --- |
|オラ | W1 - Torii のパートナーと統合者 |
|ベンタナオブジェティボ | 2025 年第 2 四半期セマナ 3 |
|タグ・デ・アーティファクト (プラネアード) | `preview-2025-04-12` |
|問題デルトラッカー | `DOCS-SORA-Preview-W1` |

## オブジェクト

1. パートナーのプレビューに関する法律と管理の手続きを確保します。
2. プロキシを準備して、招待状のテレメトリのスナップショットを試してみます。
3. プローブのチェックサムのプレビュー検証の成果物を参照します。
4. パートナーの名簿と、招待状の提出を求める最終決定。

## デグロース デ タレアス

| ID |タレア |責任者 |フェチャ限定 |エスタード |メモ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 |プレビューの期限に関する法的手続きを取得する |ドキュメント/DevRel リード -> 法務 | 2025-04-05 |コンプリート |チケット法的 `DOCS-SORA-Preview-W1-Legal` 2025-04-05; PDF 付属トラッカー。 |
| W1-P2 |プロキシのステージング ベンタナをキャプチャ 試してみてください (2025-04-10) プロキシの検証 |ドキュメント/DevRel + オペレーション | 2025-04-06 |コンプリート |参照 `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` el 2025-04-06; CLI y `.env.tryit-proxy.bak` アーカイブの転写。 |
| W1-P3 |プレビューのアーティファクト (`preview-2025-04-12`) を作成し、`scripts/preview_verify.sh` + `npm run probe:portal`、アーカイバ記述子/チェックサムを修正します。ポータルTL | 2025-04-08 |コンプリート | `artifacts/docs_preview/W1/preview-2025-04-12/` の検証ガードのアーティファクト ログ。サリダ・デ・プローブ補助トラッカー。 |
| W1-P4 |パートナーの受け入れ公式の改訂 (`DOCS-SORA-Preview-REQ-P01...P08`)、連絡先と NDA の確認 |ガバナンス連絡窓口 | 2025-04-07 |コンプリート | Las ocho solicitudes aprobadas (las ultimas dos el 2025-04-11);トラッカーの悪用。 |
| W1-P5 | Redactar の招待状のコピー (`docs/examples/docs_preview_invite_template.md` のバサド)、`<preview_tag>` および `<request_ticket>` のパートナー パートナー |ドキュメント/DevRel リード | 2025-04-08 |コンプリート | 2025 年 4 月 12 日 15:00 UTC の招待状は、芸術品を囲みます。 |

## プリフライトのチェックリスト

> コンセホ: 出力 `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` パラ出力 1-5 の自動実行 (ビルド、チェックサムの検証、ポータルのプローブ、リンク チェッカーとプロキシの実際の試行)。スクリプト レジストラは、JSON キューをログに記録し、補助的な問題のトラッカーを実行します。

1. `npm run build` (con `DOCS_RELEASE_TAG=preview-2025-04-12`) パラ再生 `build/checksums.sha256` および `build/release.json`。
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` y アーカイブ変数 `build/link-report.json` 準アル記述子。
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (`--tryit-target` 経由でターゲットを指定); `.env.tryit-proxy` の実際のロールバックを実行し、`.bak` のロールバックを保存します。
6. ログの W1 コンピューティングを実行します (ディスクリプタのチェックサム、プローブのサリダ、プロキシ Try it y スナップショット Grafana)。

## 証拠のチェックリスト

- [x] Aprobacion 法律事務所 (PDF またはチケットを添付) 付属書 `DOCS-SORA-Preview-W1`。
- [x] `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` の Grafana のスクリーンショット。
- [x] `preview-2025-04-12` のチェックサムの記述子ログ `artifacts/docs_preview/W1/`。
- [x] タイムスタンプ `invite_sent_at` の招待状リスト (バージョン ログ W1 デル トラッカー)。
- [`preview-feedback/w1/log.md`](./log.md) パートナーのフィラメントに関するフィードバック リファレンスの [x] アーティファクト (実際の 2025 年 4 月 26 日の名簿/テレメトリア/問題に関するデータ)。

Actualiza esteは、メディダ・ケ・アヴァンセン・ラス・タレアスの計画を立てています。 EL トラッカー LO 参照パラマンテナー EL ロードマップ監査可能。

## フィードバックをフルホス

1. Para cada reviewer、duplica la plantilla en
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)、
   完全なメタデータスとガードラ・ラ・コピア・ターミナダ・バホ
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。
2. テレメトリーの問題に関する招待状、チェックポイントを再開します。
   [`preview-feedback/w1/log.md`](./log.md) パラ ケ ロス レビュアー デ ゴベルナンザ プエダン リバイザー トダ ラ オラ
   リポジトリの罪。
3. Cuando lleguen は、ログに関する知識チェック、関連資料の付属品を輸出します。
   トラッカーの問題を解決します。