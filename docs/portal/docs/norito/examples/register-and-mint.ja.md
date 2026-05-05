---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 470eb5de8cd9a7f94275062d1e8c3a448a2d734bf86f650ce94a3971baa3527d
source_last_modified: "2026-04-08T09:19:38.793794+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/register-and-mint
title: ドメイン登録と資産のミント
description: 権限付きドメインの作成、資産登録、決定論的ミントを実演します。
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

権限付きドメインの作成、資産登録、決定論的ミントを実演します。

## 台帳ウォークスルー

- 宛先アカウント（例: `<i105-account-id>`）が存在することを確認し、各 SDK クイックスタートのセットアップ段階を反映します。
- `register_and_mint` エントリポイントを呼び出して ROSE 資産定義を作成し、1 トランザクションで Alice に 250 単位をミントします。
- `client.request(FindAccountAssets)` または `iroha ledger asset list all --verbose` で残高を確認し、ミントが成功したことを確かめます。

## 関連 SDK ガイド

- [Rust SDK クイックスタート](/sdks/rust)
- [Python SDK クイックスタート](/sdks/python)
- [JavaScript SDK クイックスタート](/sdks/javascript)

[Kotodama ソースをダウンロード](/norito-snippets/register-and-mint.ko)

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("<i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```
