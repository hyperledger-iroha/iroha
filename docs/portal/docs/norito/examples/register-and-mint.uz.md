<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
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
title: Domen va zarb aktivlarini ro'yxatdan o'tkazing
description: Ruxsat berilgan domen yaratish, aktivlarni ro'yxatdan o'tkazish va deterministik zarb qilishni ko'rsatadi.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

Ruxsat berilgan domen yaratish, aktivlarni ro'yxatdan o'tkazish va deterministik zarb qilishni ko'rsatadi.

## Buxgalteriya kitobi bo'yicha ko'rsatmalar

- Har bir SDK tezkor ishga tushirishda sozlash bosqichini aks ettirgan holda maqsadli hisob qaydnomasi (masalan, Alice uchun `<i105-account-id>`) mavjudligiga ishonch hosil qiling.
- ROSE aktivi ta'rifini yaratish uchun `register_and_mint` kirish nuqtasini chaqiring va bitta tranzaksiyada Elisga 250 birlik bering.
- Yalpiz muvaffaqiyatli bo'lganligini tasdiqlash uchun `client.request(FindAccountAssets)` yoki `iroha_cli ledger asset list --account <i105-account-id>` orqali balanslarni tekshiring.

## Tegishli SDK qo'llanmalari

- [Rust SDK tezkor ishga tushirish](/sdks/rust)
- [Python SDK tezkor ishga tushirish](/sdks/python)
- [JavaScript SDK tezkor ishga tushirish](/sdks/javascript)

[Kotodama manbasini yuklab oling](/norito-snippets/register-and-mint.ko)

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