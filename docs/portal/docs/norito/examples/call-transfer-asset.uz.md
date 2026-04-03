<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dcd8de175a7c5172158a03e1a25b254c90a11e62c173f95b8d9e4a387df6ba09
source_last_modified: "2026-03-26T13:01:47.372931+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/call-transfer-asset
title: Kotodama dan xost uzatishni chaqirish
description: Kotodama kirish nuqtasi qanday qilib ichki metadata tekshiruvi bilan `transfer_asset` xostiga qo'ng'iroq qilishi mumkinligini ko'rsatadi.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Kotodama kirish nuqtasi qanday qilib ichki metadata tekshiruvi bilan `transfer_asset` xostiga qo'ng'iroq qilishi mumkinligini ko'rsatadi.

## Buxgalteriya kitobi bo'yicha ko'rsatmalar

- Shartnoma organini (masalan, shartnoma hisobi uchun `<i105-account-id>`) u o'tkazadigan aktiv bilan moliyalashtiring va vakolatga `CanTransfer` rolini yoki unga tenglashtirilgan ruxsatni beradi.
- Kontrakt hisobidan Bobga (`<i105-account-id>`) 5 birlikni o'tkazish uchun `call_transfer_asset` kirish nuqtasiga qo'ng'iroq qiling, zanjirdagi avtomatlashtirish xost qo'ng'iroqlarini o'rash usulini aks ettiradi.
- `FindAccountAssets` yoki `iroha_cli ledger asset list --account <i105-account-id>` orqali balanslarni tekshiring va metadata himoyachisi uzatish kontekstini qayd etganligini tasdiqlash uchun voqealarni tekshiring.

## Tegishli SDK qo'llanmalari

- [Rust SDK tezkor ishga tushirish](/sdks/rust)
- [Python SDK tezkor ishga tushirish](/sdks/python)
- [JavaScript SDK tezkor ishga tushirish](/sdks/javascript)

[Kotodama manbasini yuklab oling](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```