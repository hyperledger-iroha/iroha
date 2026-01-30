---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: faf4993886d577a814cddb4dee2f4ccd0b47fbea4d3d54b769db86d861a8ca95
source_last_modified: "2025-11-04T12:26:02.939454+00:00"
translation_last_reviewed: 2026-01-30
---

単一の公開エントリポイントと状態ハンドルを備えた、最小構成の Kotodama コントラクトの足場。

## 台帳ウォークスルー

- `koto_compile --abi 1` でコントラクトをコンパイルします。[Norito 入門](/norito/getting-started#1-compile-a-kotodama-contract) の手順に従うか、`cargo test -p ivm developer_portal_norito_snippets_compile` を使います。
- `ivm_run` / `developer_portal_norito_snippets_run` でバイトコードをローカルにスモークテストし、`info!` ログと初期 syscall を確認してからノードに触れます。
- `iroha_cli app contracts deploy` でアーティファクトをデプロイし、[Norito 入門](/norito/getting-started#4-deploy-via-iroha_cli) の手順でマニフェストを確認します。

## 関連 SDK ガイド

- [Rust SDK クイックスタート](/sdks/rust)
- [Python SDK クイックスタート](/sdks/python)
- [JavaScript SDK クイックスタート](/sdks/javascript)

[Kotodama ソースをダウンロード](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
