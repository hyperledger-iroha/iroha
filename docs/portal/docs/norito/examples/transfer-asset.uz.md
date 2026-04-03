<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 479d552d0f641875518c62059be1084af6ddf99213662a753c73ea57512b8e5f
source_last_modified: "2026-04-02T18:24:28.189405+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/transfer-asset
title: Hisoblar o'rtasida aktivlarni o'tkazish
description: SDK tezkor ishga tushirish va buxgalteriya hisobi bo'yicha ko'rsatmalarni aks ettiruvchi to'g'ridan-to'g'ri aktivlarni uzatish ish jarayoni.
source: examples/transfer/transfer.ko
---

SDK tezkor ishga tushirish va buxgalteriya hisobi bo'yicha ko'rsatmalarni aks ettiruvchi to'g'ridan-to'g'ri aktivlarni uzatish ish jarayoni.

## Buxgalteriya kitobi bo'yicha ko'rsatmalar

- Elisni maqsadli aktiv bilan oldindan moliyalashtiring (masalan, “roʻyxatdan oʻtish va zarb qilish” snippeti yoki SDK tezkor boshlash oqimlari orqali).
- `AssetTransferRole` ruxsatini qondirib, Elisdan Bobga 10 birlikni ko'chirish uchun `do_transfer` kirish nuqtasini bajaring.
- Balanslarni so'rang (`FindAccountAssets`, `iroha_cli ledger asset list`) yoki o'tkazish natijalarini kuzatish uchun quvur hodisalariga obuna bo'ling.

## Tegishli SDK qo'llanmalari

- [Rust SDK tezkor ishga tushirish](/sdks/rust)
- [Python SDK tezkor ishga tushirish](/sdks/python)
- [JavaScript SDK tezkor ishga tushirish](/sdks/javascript)

[Kotodama manbasini yuklab oling](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1QG1シタ3vN7ヒzトヘcミLKDCAイ5クエjヤリ2uトユmキユルeJBJW7X2N7"),
      account!("sorauロ1NksツJZミLツスjヨrUphCSホ8Wノスマチモr3ムLセヌヒYqwフノFTMDQE"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```