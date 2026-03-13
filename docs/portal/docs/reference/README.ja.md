---
lang: ja
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-11-08T06:08:33.073497+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: リファレンス インデックス
slug: /reference
---

このセクションは、Iroha の「仕様として読む」資料を集約します。これらのページは、ガイドやチュートリアルが進化しても安定したままです。

## 現在利用可能

- **Norito コーデック概要** - `reference/norito-codec.md` は、ポータルの表が整備されるまで、権威ある `norito.md` 仕様へ直接リンクします。
- **Torii OpenAPI** - `/reference/torii-openapi` は Redoc を使って最新の Torii REST 仕様を描画します。`npm run sync-openapi -- --version=current --latest` で再生成します（`--mirror=<label>` を追加するとスナップショットを追加の履歴バージョンに複製できます）。
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v2/mcp`.
- **設定テーブル** - パラメータの完全なカタログは `docs/source/references/configuration.md` にあります。ポータルが自動インポートを提供するまで、その Markdown ファイルで正確な既定値と環境上書きを参照してください。
- **ドキュメントのバージョニング** - ナビバーのバージョンドロップダウンは `npm run docs:version -- <label>` で作成した凍結スナップショットを表示し、リリース間のガイダンス比較を容易にします。
