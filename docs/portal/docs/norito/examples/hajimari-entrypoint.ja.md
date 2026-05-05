---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 93b3329a32d69ca67bd267256a57c40d6619d07e1e39b2ec912fba3621cec123
source_last_modified: "2026-04-08T09:19:38.793123+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/hajimari-entrypoint
title: Hajimari エントリポイントの骨組み
description: 単一の公開エントリポイントと状態ハンドルを備えた、最小構成の Kotodama コントラクトの足場。
source: crates/ivm/docs/examples/01_hajimari.ko
---

単一の公開エントリポイントと状態ハンドルを備えた、最小構成の Kotodama コントラクトの足場。

## 台帳ウォークスルー

- `koto_compile --abi 1` でコントラクトをコンパイルします。[Norito 入門](/norito/getting-started#1-compile-a-kotodama-contract) の手順に従うか、`cargo test -p ivm developer_portal_norito_snippets_compile` を使います。
- `ivm_run` / `developer_portal_norito_snippets_run` でバイトコードをローカルにスモークテストし、`info!` ログと初期 syscall を確認してからノードに触れます。
- `iroha app contracts deploy` でアーティファクトをデプロイし、[Norito 入門](/norito/getting-started#4-deploy-via-iroha) の手順でマニフェストを確認します。

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
