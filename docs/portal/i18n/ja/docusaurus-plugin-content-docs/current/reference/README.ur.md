---
lang: ur
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c82723b3555009a3348e409e0614e7cf2dde682b74cc7bf116be05885e65a691
source_last_modified: "2025-11-04T12:12:47.580487+00:00"
translation_last_reviewed: 2026-01-30
---

このセクションは、Iroha の「仕様として読む」資料を集約します。これらのページは、ガイドやチュートリアルが進化しても安定したままです。

## 現在利用可能

- **Norito コーデック概要** - `reference/norito-codec.md` は、ポータルの表が整備されるまで、権威ある `norito.md` 仕様へ直接リンクします。
- **Torii OpenAPI** - `/reference/torii-openapi` は Redoc を使って最新の Torii REST 仕様を描画します。`npm run sync-openapi -- --version=current --latest` で再生成します（`--mirror=<label>` を追加するとスナップショットを追加の履歴バージョンに複製できます）。
- **設定テーブル** - パラメータの完全なカタログは `docs/source/references/configuration.md` にあります。ポータルが自動インポートを提供するまで、その Markdown ファイルで正確な既定値と環境上書きを参照してください。
- **ドキュメントのバージョニング** - ナビバーのバージョンドロップダウンは `npm run docs:version -- <label>` で作成した凍結スナップショットを表示し、リリース間のガイダンス比較を容易にします。
