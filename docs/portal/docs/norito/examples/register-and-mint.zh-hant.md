<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e686495c642a08740504c4bb5f88e623c89a896787388b61e4451f550f87af6
source_last_modified: "2026-03-26T13:01:47.376183+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/register-and-mint
title: 註冊域名和鑄幣資產
description: 演示許可的網域創建、資產註冊和確定性鑄幣。
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

演示許可的網域創建、資產註冊和確定性鑄幣。

## 帳本演練

- 確保目標帳戶（例如，Alice 的 `<i105-account-id>`）存在，鏡像每個 SDK 快速入門中的設定階段。
- 呼叫 `register_and_mint` 入口點來建立 ROSE 資產定義並在一筆交易中向 Alice 鑄造 250 個單位。
- 透過`client.request(FindAccountAssets)`或`iroha_cli ledger asset list --account <i105-account-id>`驗證餘額以確認鑄幣成功。

## 相關SDK指南

- [Rust SDK 快速入門](/sdks/rust)
- [Python SDK 快速入門](/sdks/python)
- [JavaScript SDK 快速入門](/sdks/javascript)

[下載Kotodama原始碼](/norito-snippets/register-and-mint.ko)

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  #[access(read="*", write="*")]
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```