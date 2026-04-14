<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
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
title: Домэйн болон гаалийн хөрөнгийг бүртгүүлэх
description: Зөвшөөрөгдсөн домэйн үүсгэх, хөрөнгийн бүртгэл, детерминист зардлыг харуулна.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

Зөвшөөрөгдсөн домэйн үүсгэх, хөрөнгийн бүртгэл, детерминист зардлыг харуулна.

## Бүртгэлийн дэвтэр

- SDK-ийн хурдан эхлүүлэх бүрт тохируулах үе шатыг тусгаж, очих хаяг (жишээ нь, `<i105-account-id>` for Alice) байгаа эсэхийг шалгаарай.
- ROSE хөрөнгийн тодорхойлолтыг үүсгэхийн тулд `register_and_mint` нэвтрэх цэгийг дуудаж, нэг гүйлгээнд 250 нэгжийг Алис руу илгээнэ үү.
- Үлдэгдлийг `client.request(FindAccountAssets)` эсвэл `iroha_cli ledger asset list --account <i105-account-id>`-ээр шалгана уу.

## Холбогдох SDK гарын авлага

- [Зэв SDK хурдан эхлүүлэх](/sdks/rust)
- [Python SDK хурдан эхлүүлэх](/sdks/python)
- [JavaScript SDK хурдан эхлүүлэх](/sdks/javascript)

[Kotodama эх сурвалжийг татаж авах](/norito-snippets/register-and-mint.ko)

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